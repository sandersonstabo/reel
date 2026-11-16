use std::{
    cell::RefCell,
    fs,
    sync::atomic::{AtomicU32, Ordering},
};

use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM},
        Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{CreateMutexW, GetCurrentProcess, SetProcessWorkingSetSize},
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, VK_F8},
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
                NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
                NOTIFYICONDATAW_0, ShellExecuteW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                MSG, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SW_SHOWNORMAL,
                SetTimer, TranslateMessage, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY,
                WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
            },
        },
    },
    core::{HSTRING, PCWSTR, w},
};

use super::{
    autostart,
    icon::Icons,
    menu::{self, Choice, Snapshot},
};
use crate::{
    detect::{
        classify::{KnownGames, classify},
        known, probe,
    },
    log, paths,
    session::{Action, Cue, Event, Game, Mode, step},
    worker::Worker,
};

const WM_TRAY: u32 = WM_APP + 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const HOTKEY_TOGGLE: i32 = 1;
const TIMER_TICK: usize = 1;
const TICK_MS: u32 = 1000;

static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

struct App {
    window: HWND,
    mode: Mode,
    worker: Option<Worker>,
    finishing: Vec<Worker>,
    known: KnownGames,
    last_pid: Option<u32>,
    icons: Icons,
    hotkey: bool,
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

impl App {
    fn status(&self) -> String {
        let base = match &self.mode {
            Mode::Off => "Reel: off (F8 to turn on)".to_string(),
            Mode::Watching { .. } => "Reel: waiting for a game".to_string(),
            Mode::Recording(game) => format!("Reel: recording {}", game.name),
        };
        match self.hotkey {
            true => base,
            false => format!("{base} (F8 unavailable)"),
        }
    }

    fn icon(&self) -> windows::Win32::UI::WindowsAndMessaging::HICON {
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
                    log::line(&format!("stop {}", worker.output().display()));
                    self.finishing.push(worker.stop());
                }
            }
            Action::Cue(sound) => cue(sound),
        }
    }

    fn start(&mut self, game: &Game) {
        let output = paths::output_for(&game.name);
        match Worker::spawn(game, &output) {
            Ok(worker) => {
                log::line(&format!("start {} -> {}", game.name, output.display()));
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

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            status: self.status(),
            enabled: self.mode != Mode::Off,
            autostart: autostart::enabled(),
        }
    }

    fn shutdown(&mut self) {
        self.perform(Action::Stop);
        let _ =unsafe { Shell_NotifyIconW(NIM_DELETE, &self.notify_data()) };
    }
}

fn open_recordings() {
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

fn choose(window: HWND) {
    let Some(snapshot) = with_app(|app| app.snapshot()) else {
        return;
    };
    match menu::show(window, &snapshot) {
        Some(Choice::Toggle) => {
            with_app(|app| app.dispatch(Event::Toggle));
        }
        Some(Choice::OpenRecordings) => open_recordings(),
        Some(Choice::Autostart) => autostart::set(!snapshot.autostart),
        Some(Choice::Quit) => {
            let _ = unsafe { DestroyWindow(window) };
        }
        None => {}
    }
}

extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_HOTKEY if wparam.0 == HOTKEY_TOGGLE as usize => {
            with_app(|app| app.dispatch(Event::Toggle));
            LRESULT(0)
        }
        WM_TIMER => {
            with_app(App::tick);
            LRESULT(0)
        }
        WM_TRAY => {
            match (lparam.0 & 0xffff) as u32 {
                WM_CONTEXTMENU | NIN_SELECT | NIN_KEYSELECT => choose(window),
                _ => {}
            }
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
    let hotkey = unsafe { RegisterHotKey(Some(window), HOTKEY_TOGGLE, MOD_NOREPEAT, u32::from(VK_F8.0)) }.is_ok();
    if !hotkey {
        log::line("F8 is already registered by another program");
    }
    unsafe { SetTimer(Some(window), TIMER_TICK, TICK_MS, None) };
    APP.with(|cell| {
        cell.replace(Some(App {
            window,
            mode: Mode::Watching { skip: None },
            worker: None,
            finishing: Vec::new(),
            known: KnownGames::new(),
            last_pid: None,
            icons: Icons::new(),
            hotkey,
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
