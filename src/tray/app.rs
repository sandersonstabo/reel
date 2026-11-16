use std::{
    cell::RefCell,
    fs,
    sync::atomic::{AtomicU32, Ordering},
};

use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM},
        Media::{
            Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT},
            MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFStartup},
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{CreateMutexW, GetCurrentProcess, SetProcessWorkingSetSize},
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            Input::KeyboardAndMouse::{
                HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_F8,
            },
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
                NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
                NOTIFYICONDATAW_0, ShellExecuteW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                GetSystemMetrics, HICON, MSG, PostQuitMessage, RegisterClassW,
                RegisterWindowMessageW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOWNORMAL, SetTimer,
                TranslateMessage, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION, WM_HOTKEY, WM_TIMER,
                WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
            },
        },
    },
    core::{HSTRING, PCWSTR, w},
};

use super::{
    autostart,
    icon::Icons,
    menu::{self, Choice, Item, entry},
};
use crate::{
    detect::{
        classify::{KnownGames, classify},
        known, probe,
    },
    log, paths,
    record::{encode::video, plan},
    session::{Action, Cue, Event, Game, Mode, step},
    settings::{ClipKey, ClipLength, Codec, FrameRate, ResolutionCap, Settings, SizeTarget, store},
    worker::Worker,
};

const WM_TRAY: u32 = WM_APP + 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const HOTKEY_TOGGLE: i32 = 1;
const HOTKEY_CLIP: i32 = 2;
const TIMER_TICK: usize = 1;
const TICK_MS: u32 = 1000;

static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

struct App {
    window: HWND,
    settings: Settings,
    mode: Mode,
    worker: Option<Worker>,
    finishing: Vec<Worker>,
    known: KnownGames,
    last_pid: Option<u32>,
    icons: Icons,
    toggle_key: bool,
    clip_key: bool,
    codecs: Option<Vec<(Codec, bool)>>,
}

fn with_app<T>(f: impl FnOnce(&mut App) -> T) -> Option<T> {
    APP.with(|cell| cell.borrow_mut().as_mut().map(f))
}

fn trim() {
    let _ = unsafe { SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) };
}

fn cue(cue: Cue) {
    let alias = match cue {
        Cue::On => w!("DeviceConnect"),
        Cue::Off => w!("DeviceDisconnect"),
        Cue::Saved => w!("SystemAsterisk"),
        Cue::Unavailable => w!("SystemExclamation"),
    };
    let _ = unsafe { PlaySoundW(alias, None, SND_ALIAS | SND_ASYNC | SND_NODEFAULT) };
}

fn wide<const N: usize>(text: &str) -> [u16; N] {
    let mut buffer = [0u16; N];
    text.encode_utf16()
        .take(N - 1)
        .enumerate()
        .for_each(|(index, unit)| buffer[index] = unit);
    buffer
}

fn screen() -> plan::Resolution {
    plan::native(
        unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(2) as u32,
        unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(2) as u32,
    )
}

fn register(window: HWND, id: i32, key: u16) -> bool {
    unsafe { RegisterHotKey(Some(window), id, HOT_KEY_MODIFIERS(MOD_NOREPEAT.0), u32::from(key)) }.is_ok()
}

fn mode_for(settings: &Settings) -> Mode {
    match settings.enabled {
        true => Mode::Watching { skip: None },
        false => Mode::Off,
    }
}

impl App {
    fn status(&self) -> String {
        let base = match &self.mode {
            Mode::Off => "Reel: off (F8 to turn on)".to_string(),
            Mode::Watching { .. } => "Reel: waiting for a game".to_string(),
            Mode::Recording(game) => format!(
                "Reel: buffering {} ({} to clip)",
                game.name,
                self.settings.clip_key.label()
            ),
        };
        let missing: Vec<&str> = [(!self.toggle_key).then_some("F8"), (!self.clip_key).then_some(self.settings.clip_key.label())]
            .into_iter()
            .flatten()
            .collect();
        match missing.is_empty() {
            true => base,
            false => format!("{base} ({} taken by another app)", missing.join(", ")),
        }
    }

    fn icon(&self) -> HICON {
        match self.mode {
            Mode::Off => self.icons.off,
            Mode::Watching { .. } => self.icons.watching,
            Mode::Recording(_) => self.icons.recording,
        }
    }

    fn notify_data(&self) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.window,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_TRAY,
            hIcon: self.icon(),
            szTip: wide(&self.status()),
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            ..Default::default()
        }
    }

    fn add_icon(&self) {
        let data = self.notify_data();
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &data);
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
        }
    }

    fn refresh_icon(&self) {
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.notify_data()) };
    }

    fn dispatch(&mut self, event: Event) {
        let (mode, actions) = step(std::mem::replace(&mut self.mode, Mode::Off), event);
        let changed = mode != self.mode;
        self.mode = mode;
        actions.into_iter().for_each(|action| self.perform(action));
        if changed {
            self.refresh_icon();
            trim();
        }
    }

    fn perform(&mut self, action: Action) {
        match action {
            Action::Start(game) => self.start(&game),
            Action::Stop => {
                if let Some(worker) = self.worker.take() {
                    self.finishing.push(worker.stop());
                }
            }
            Action::Save(game) => {
                let output = paths::output_for(&game.name);
                let sent = self.worker.as_mut().map(|worker| worker.save(&output));
                if let Some(Err(error)) = sent {
                    log::line(&format!("could not request clip: {error}"));
                }
            }
            Action::Cue(sound) => cue(sound),
        }
    }

    fn start(&mut self, game: &Game) {
        match Worker::spawn(game) {
            Ok(worker) => {
                log::line(&format!("buffering {}", game.name));
                self.worker = Some(worker);
            }
            Err(error) => log::line(&format!("spawn failed for {}: {error}", game.name)),
        }
    }

    fn focused_game(&mut self) -> Option<Game> {
        let foreground = probe::foreground()?;
        if self.last_pid != Some(foreground.pid) {
            self.known = known::load();
            self.last_pid = Some(foreground.pid);
        }
        classify(&foreground.candidate(), &self.known)
            .is_game()
            .then(|| foreground.game())
    }

    fn tick(&mut self) {
        self.finishing.retain_mut(|worker| !worker.exited());
        let event = match &self.mode {
            Mode::Off => None,
            Mode::Watching { .. } => self.focused_game().map(Event::GameFocused),
            Mode::Recording(game) => {
                match (probe::alive(game), self.worker.as_mut().map(Worker::exited)) {
                    (false, _) => Some(Event::GameClosed),
                    (true, Some(false)) => None,
                    (true, _) => Some(Event::RecorderFailed),
                }
            }
        };
        if let Some(event) = event {
            self.dispatch(event);
        }
    }

    fn toggle(&mut self) {
        self.dispatch(Event::Toggle);
        self.settings.enabled = self.mode != Mode::Off;
        store::save(&self.settings);
    }

    fn rebind_clip_key(&mut self, previous: ClipKey) {
        let _ = unsafe { UnregisterHotKey(Some(self.window), HOTKEY_CLIP) };
        self.clip_key = register(self.window, HOTKEY_CLIP, self.settings.clip_key.virtual_key());
        if !self.clip_key {
            log::line(&format!(
                "{} is taken by another app; {} was in use before",
                self.settings.clip_key.label(),
                previous.label()
            ));
        }
    }

    fn update(&mut self, change: impl FnOnce(Settings) -> Settings) {
        let previous = self.settings;
        self.settings = change(previous);
        store::save(&self.settings);
        if previous.clip_key != self.settings.clip_key {
            self.rebind_clip_key(previous.clip_key);
        }
        self.dispatch(Event::SettingsChanged);
        self.refresh_icon();
    }

    fn codecs(&mut self) -> Vec<(Codec, bool)> {
        let probed = self.codecs.get_or_insert_with(|| {
            let _ = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) };
            Codec::ALL.into_iter().map(|codec| (codec, video::available(codec))).collect()
        });
        probed.clone()
    }

    fn items(&mut self) -> Vec<Item> {
        let settings = self.settings;
        let native = screen();
        let preview = |target: SizeTarget| plan::label(&plan::plan(native, &Settings { size_target: target, ..settings }));
        let recording = matches!(self.mode, Mode::Recording(_));
        let codecs = self.codecs();
        vec![
            Item::Note(self.status()),
            Item::Separator,
            entry("Clipping on\tF8", settings.enabled, Choice::Toggle),
            Item::Entry {
                label: format!("Save clip\t{}", settings.clip_key.label()),
                checked: false,
                enabled: recording,
                choice: Choice::SaveClip,
            },
            Item::Separator,
            Item::Submenu {
                label: format!("Clip length: {}", settings.clip_length.label()),
                items: ClipLength::ALL
                    .into_iter()
                    .map(|length| entry(length.label(), length == settings.clip_length, Choice::Length(length)))
                    .collect(),
            },
            Item::Submenu {
                label: format!("Size target: {}", match settings.size_target {
                    SizeTarget::Unlimited => "none".to_string(),
                    SizeTarget::Megabytes(megabytes) => format!("{megabytes} MB"),
                }),
                items: SizeTarget::ALL
                    .into_iter()
                    .map(|target| {
                        let label = match target {
                            SizeTarget::Unlimited => format!("None (best quality, {})", preview(target)),
                            SizeTarget::Megabytes(megabytes) => format!("{megabytes} MB ({})", preview(target)),
                        };
                        entry(label, target == settings.size_target, Choice::Target(target))
                    })
                    .collect(),
            },
            Item::Submenu {
                label: format!("Resolution: {}", match settings.resolution.height() {
                    None => format!("native ({}p)", native.height),
                    Some(height) => format!("{height}p"),
                }),
                items: ResolutionCap::ALL
                    .into_iter()
                    .map(|cap| {
                        let (label, enabled) = match cap.height() {
                            None => (format!("Native ({}p)", native.height), true),
                            Some(height) => (format!("{height}p"), height <= native.height),
                        };
                        Item::Entry {
                            label,
                            checked: cap == settings.resolution,
                            enabled,
                            choice: Choice::Resolution(cap),
                        }
                    })
                    .collect(),
            },
            Item::Submenu {
                label: format!("Frame rate: {} fps", settings.frame_rate.fps()),
                items: FrameRate::ALL
                    .into_iter()
                    .map(|rate| entry(format!("{} fps", rate.fps()), rate == settings.frame_rate, Choice::FrameRate(rate)))
                    .collect(),
            },
            Item::Submenu {
                label: format!("Codec: {}", settings.codec.label()),
                items: codecs
                    .into_iter()
                    .map(|(codec, supported)| Item::Entry {
                        label: match supported {
                            true => codec.label().to_string(),
                            false => format!("{} (not supported by this GPU)", codec.label()),
                        },
                        checked: codec == settings.codec,
                        enabled: supported,
                        choice: Choice::Codec(codec),
                    })
                    .collect(),
            },
            Item::Submenu {
                label: format!("Clip key: {}", settings.clip_key.label()),
                items: ClipKey::ALL
                    .into_iter()
                    .map(|key| entry(key.label(), key == settings.clip_key, Choice::Key(key)))
                    .collect(),
            },
            Item::Separator,
            entry("Start with Windows", autostart::enabled(), Choice::Autostart),
            entry("Open clips folder", false, Choice::OpenClips),
            Item::Separator,
            entry("Quit", false, Choice::Quit),
        ]
    }

    fn choose(&mut self, choice: Choice) {
        match choice {
            Choice::Toggle => self.toggle(),
            Choice::SaveClip => self.dispatch(Event::SaveClip),
            Choice::Length(clip_length) => self.update(|settings| Settings { clip_length, ..settings }),
            Choice::Target(size_target) => self.update(|settings| Settings { size_target, ..settings }),
            Choice::Resolution(resolution) => self.update(|settings| Settings { resolution, ..settings }),
            Choice::FrameRate(frame_rate) => self.update(|settings| Settings { frame_rate, ..settings }),
            Choice::Codec(codec) => self.update(|settings| Settings { codec, ..settings }),
            Choice::Key(clip_key) => self.update(|settings| Settings { clip_key, ..settings }),
            Choice::Autostart => autostart::set(!autostart::enabled()),
            Choice::OpenClips => open_clips(),
            Choice::Quit => {}
        }
    }

    fn shutdown(&mut self) {
        self.perform(Action::Stop);
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.notify_data()) };
    }
}

fn open_clips() {
    let directory = paths::recordings();
    let _ = fs::create_dir_all(&directory);
    let _ = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(directory.as_os_str()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
}

fn open_menu(window: HWND) {
    let Some(items) = with_app(App::items) else {
        return;
    };
    let picked = menu::show(window, &items);
    trim();
    match picked {
        Some(Choice::Quit) => {
            let _ = unsafe { DestroyWindow(window) };
        }
        Some(choice) => {
            with_app(|app| app.choose(choice));
        }
        None => {}
    }
}

extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_HOTKEY => {
            match wparam.0 as i32 {
                HOTKEY_TOGGLE => {
                    with_app(App::toggle);
                }
                HOTKEY_CLIP => {
                    with_app(|app| app.dispatch(Event::SaveClip));
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            with_app(App::tick);
            LRESULT(0)
        }
        WM_TRAY => {
            match (lparam.0 & 0xffff) as u32 {
                WM_CONTEXTMENU | NIN_SELECT | NIN_KEYSELECT => open_menu(window),
                _ => {}
            }
            LRESULT(0)
        }
        WM_ENDSESSION if wparam.0 != 0 => {
            with_app(App::shutdown);
            LRESULT(0)
        }
        WM_DESTROY => {
            with_app(App::shutdown);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        message if message != 0 && message == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            with_app(|app| app.add_icon());
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn create_window() -> Option<HWND> {
    let instance = unsafe { GetModuleHandleW(None) }.ok()?;
    let class = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance.into(),
        lpszClassName: w!("Reel.Tray"),
        ..Default::default()
    };
    match unsafe { RegisterClassW(&class) } {
        0 => None,
        _ => unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("Reel.Tray"),
                w!("Reel"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }
        .ok(),
    }
}

pub fn main() -> i32 {
    let _instance = unsafe { CreateMutexW(None, true, w!("Local\\Reel.Tray")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return 0;
    }
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let Some(window) = create_window() else {
        log::line("could not create tray window");
        return 1;
    };
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }, Ordering::Relaxed);
    let settings = store::load();
    autostart::follow_current_exe();
    let toggle_key = register(window, HOTKEY_TOGGLE, VK_F8.0);
    let clip_key = register(window, HOTKEY_CLIP, settings.clip_key.virtual_key());
    unsafe { SetTimer(Some(window), TIMER_TICK, TICK_MS, None) };
    APP.with(|cell| {
        cell.replace(Some(App {
            window,
            settings,
            mode: mode_for(&settings),
            worker: None,
            finishing: Vec::new(),
            known: KnownGames::new(),
            last_pid: None,
            icons: Icons::new(),
            toggle_key,
            clip_key,
            codecs: None,
        }))
    });
    with_app(|app| {
        app.add_icon();
        app.tick();
    });
    log::line("tray started");
    trim();
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    APP.with(|cell| cell.replace(None));
    0
}
