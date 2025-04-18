#![allow(warnings)]

use retour::static_detour;

use egui::{
    Align2, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, FontTweak,
    ImageSource, Key, Modifiers, Pos2, Rect, RichText, ScrollArea, Slider, Stroke, StrokeKind,
    TextureId, Vec2, Widget,
};
use egui_d3d9::EguiDx9;
use std::{
    intrinsics::transmute,
    sync::{Arc, Once},
    time::Duration,
};
use windows::{
    core::{s, HRESULT, PCSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Direct3D9::{
                IDirect3DDevice9, IDirect3DSwapChain9, D3DDEVICE_CREATION_PARAMETERS,
                D3DPRESENT_PARAMETERS,
            },
            Dxgi::Common::DXGI_FORMAT,
            Gdi::RGNDATA,
        },
        System::{Console::AllocConsole, LibraryLoader::GetModuleHandleA},
        UI::WindowsAndMessaging::{
            CallWindowProcW, FindWindowA, SetWindowLongPtrA, GWLP_WNDPROC, WNDPROC,
        },
    },
};

#[no_mangle]
extern "stdcall" fn DllMain(hinst: usize, reason: u32, _reserved: *mut ()) -> i32 {
    if reason == 1 {
        std::thread::spawn(move || unsafe { main_thread(hinst) });
    }

    1
}

static mut APP: Option<EguiDx9<i32>> = None;
static mut OLD_WND_PROC: Option<WNDPROC> = None;

static_detour! {
    static PresentHook: unsafe extern "stdcall" fn(IDirect3DDevice9, *const RECT, *const RECT, HWND, *const RGNDATA) -> HRESULT;
    static ResetHook: unsafe extern "stdcall" fn(IDirect3DDevice9, *const D3DPRESENT_PARAMETERS) -> HRESULT;
}

type FnPresent = unsafe extern "stdcall" fn(
    IDirect3DDevice9,
    *const RECT,
    *const RECT,
    HWND,
    *const RGNDATA,
) -> HRESULT;
type FnReset =
    unsafe extern "stdcall" fn(IDirect3DDevice9, *const D3DPRESENT_PARAMETERS) -> HRESULT;

static mut O_PRESENT: Option<FnPresent> = None;
static mut O_RESET: Option<FnReset> = None;

fn hk_present(
    dev: IDirect3DDevice9,
    source_rect: *const RECT,
    dest_rect: *const RECT,
    window: HWND,
    rgn_data: *const RGNDATA,
) -> HRESULT {
    unsafe {
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            // let window = FindWindowA(s!("Valve001"), PCSTR(std::ptr::null()));
            let window = FindWindowA(s!("Valve001"), PCSTR(std::ptr::null()))
                .expect("unable to find valve window");

            APP = Some(EguiDx9::init(&dev, window, ui, 0, true));

            OLD_WND_PROC = Some(transmute(SetWindowLongPtrA(
                window,
                GWLP_WNDPROC,
                hk_wnd_proc as usize as _,
            )));
        });

        APP.as_mut().unwrap().present(&dev);

        PresentHook.call(dev, source_rect, dest_rect, window, rgn_data)
    }
}

fn hk_reset(
    dev: IDirect3DDevice9,
    presentation_parameters: *const D3DPRESENT_PARAMETERS,
) -> HRESULT {
    unsafe {
        APP.as_mut().unwrap().pre_reset();

        ResetHook.call(dev, presentation_parameters)
    }
}

unsafe extern "stdcall" fn hk_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    APP.as_mut().unwrap().wnd_proc(msg, wparam, lparam);

    CallWindowProcW(OLD_WND_PROC.unwrap(), hwnd, msg, wparam, lparam)
}

// most of this code is ported over from sy1ntexx's d3d11 implementation.
static mut FRAME: i32 = 0;
fn ui(ctx: &Context, i: &mut i32) {
    unsafe {
        // You should not use statics like this, it's made
        // this way for the sake of example.
        static mut UI_CHECK: bool = true;
        static mut TEXT: Option<String> = None;
        static mut VALUE: f32 = 0.;
        static mut COLOR: [f32; 3] = [0., 0., 0.];
        static ONCE: Once = Once::new();

        ONCE.call_once(|| {
            // Uncomment this to set other fonts.
            // let mut fonts = FontDefinitions::default();
            // let mut tweak = FontTweak::default();
            // fonts.font_data.insert(
            //     "my_font".to_owned(),
            //     FontData::from_static(include_bytes!("Lobster-Regular.ttf")).tweak(tweak),
            // );
            // fonts
            //     .families
            //     .get_mut(&FontFamily::Proportional)
            //     .unwrap()
            //     .insert(0, "my_font".to_owned());
            // fonts
            //     .families
            //     .get_mut(&FontFamily::Monospace)
            //     .unwrap()
            //     .push("my_font".to_owned());
            // ctx.set_fonts(fonts);
            egui_extras::install_image_loaders(ctx);
        });

        if TEXT.is_none() {
            TEXT = Some(String::from("Test"));
        }

        ctx.debug_painter().text(
            Pos2::new(0., 0.),
            Align2::LEFT_TOP,
            "Bruh",
            FontId::default(),
            Color32::RED,
        );

        egui::containers::Window::new("Main menu").show(ctx, |ui| {
            ctx.settings_ui(ui);
            ui.label(RichText::new("Test").color(Color32::BLACK));
            ui.label(RichText::new("Other").color(Color32::WHITE));
            ui.separator();

            ui.label(RichText::new(format!("I: {}", *i)).color(Color32::LIGHT_RED));

            let input = ctx.input(|input| input.pointer.clone());
            ui.label(format!(
                "X1: {} X2: {}",
                input.button_down(egui::PointerButton::Extra1),
                input.button_down(egui::PointerButton::Extra2)
            ));

            let mods = ui.input(|input| input.modifiers);
            ui.label(format!(
                "Ctrl: {} Shift: {} Alt: {}",
                mods.ctrl, mods.shift, mods.alt
            ));

            if ui.input(|input| {
                input.modifiers.matches(Modifiers::CTRL) && input.key_pressed(Key::R)
            }) {
                println!("Pressed");
            }

            unsafe {
                ui.checkbox(&mut UI_CHECK, "Some checkbox");
                ui.text_edit_singleline(TEXT.as_mut().unwrap());
                ScrollArea::vertical().max_height(200.).show(ui, |ui| {
                    for i in 1..=100 {
                        ui.label(format!("Label: {}", i));
                    }
                });

                Slider::new(&mut VALUE, -1.0..=1.0).ui(ui);

                ui.color_edit_button_rgb(&mut COLOR);
            }

            ui.label(format!(
                "{:?}",
                &ui.input(|input| input.pointer.button_down(egui::PointerButton::Primary))
            ));
            if ui.button("You can't click me yet").clicked() {
                *i += 1;
            }
        });

        egui::Window::new("Image").show(ctx, |ui| unsafe {
            const IMG: ImageSource = egui::include_image!("logo.bmp");

            ui.image(IMG);
        });

        egui::Window::new("xd").show(ctx, |ui| unsafe {
            ctx.memory_ui(ui);
        });

        egui::Window::new("stuff").show(ctx, |ui| unsafe {
            ctx.inspection_ui(ui);
        });

        ctx.debug_painter().rect(
            Rect {
                min: Pos2::new(200.0, 200.0),
                max: Pos2::new(250.0, 250.0),
            },
            10.0,
            Color32::from_rgba_premultiplied(255, 0, 0, 150),
            Stroke::NONE,
            StrokeKind::Inside,
        );

        // this is supposed to be color channel testing to identify if any channels have been misplaced
        ctx.debug_painter().circle(
            Pos2::new(350.0, 350.0),
            35.0,
            Color32::from_rgba_premultiplied(255, 0, 0, 0),
            Stroke::NONE,
        );

        ctx.debug_painter().circle(
            Pos2::new(450.0, 350.0),
            35.0,
            Color32::from_rgba_premultiplied(0, 255, 0, 0),
            Stroke::NONE,
        );

        ctx.debug_painter().circle(
            Pos2::new(550.0, 350.0),
            35.0,
            Color32::from_rgba_premultiplied(0, 0, 255, 0),
            Stroke::NONE,
        );

        ctx.debug_painter().circle(
            Pos2::new(650.0, 350.0),
            35.0,
            Color32::from_rgba_premultiplied(0, 0, 0, 255),
            Stroke::new(5f32, Color32::from_rgba_premultiplied(0, 0, 255, 255)),
        );
    }
}

unsafe fn main_thread(_hinst: usize) {
    unsafe {
        AllocConsole();
    }

    unsafe {
        // for valve games
        if FindWindowA(s!("Valve001"), PCSTR(std::ptr::null())).is_ok_and(|x| !x.is_invalid()) {
            while GetModuleHandleA(s!("serverbrowser.dll")).is_err() {
                std::thread::sleep(Duration::new(0, 100_000_000));
            }
        }
    }

    let methods = shroud::directx9::methods().unwrap();

    let reset = methods.device_vmt()[16];
    let present = methods.device_vmt()[17];

    eprintln!("Present: {:X}", present as usize);
    eprintln!("Reset: {:X}", reset as usize);

    let present: FnPresent = std::mem::transmute(present);
    let reset: FnReset = std::mem::transmute(reset);

    PresentHook
        .initialize(present, hk_present)
        .unwrap()
        .enable()
        .unwrap();

    ResetHook
        .initialize(reset, hk_reset)
        .unwrap()
        .enable()
        .unwrap();
}
