use std::cell::Cell;
use std::rc::Rc;

use overlay::OverlayTarget;

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .parse_default_env()
        .init();

    log::info!("Initialize overlay");
    let overlay = overlay::init(overlay::OverlayOptions {
        title: "Task Manager Overlay".to_string(),
        target: OverlayTarget::WindowTitle("Task Manager".into()),
        register_fonts_callback: None,
    })?;
    let mut text_input = Default::default();
    let mut run_loop = true;

    let capture_blocked = Rc::new(Cell::new(true));
    let capture_dirty = Rc::new(Cell::new(true));
    let cb_update = capture_blocked.clone();
    let cd_update = capture_dirty.clone();
    let cb_render = capture_blocked.clone();
    let cd_render = capture_dirty.clone();

    overlay.main_loop(
        move |controller| {
            controller.toggle_debug_overlay(true);
            if cd_update.get() {
                controller.toggle_screen_capture_visibility(!cb_update.get());
                cd_update.set(false);
            }
            true
        },
        move |ui, unicode_text| {
            ui.window("Dummy Window")
                .resizable(true)
                .movable(true)
                .build(|| {
                    unicode_text.text("Taskmanager Overlay!");
                    unicode_text.text(format!("FPS: {:.2}", ui.io().framerate));

                    ui.input_text("Test-Input", &mut text_input).build();
                    unicode_text.register_unicode_text(&text_input);

                    if ui.button("Close") {
                        run_loop = false
                    }

                    let label = if cb_render.get() {
                        "Disable protection (allow capture)"
                    } else {
                        "Enable protection (block capture)"
                    };
                    if ui.button(label) {
                        cb_render.set(!cb_render.get());
                        cd_render.set(true);
                    }

                    unicode_text.text("Привет, мир!");
                    unicode_text.text("Chào thế giới!");
                    unicode_text.text("Chào thế giới!");
                    unicode_text.text("ສະ​ບາຍ​ດີ​ຊາວ​ໂລກ!");
                    unicode_text.text("Салом Ҷаҳон!");
                    unicode_text.text("こんにちは世界!");
                    unicode_text.text("你好世界!");
                    unicode_text.text("﷽, ♛ LAZ ♛,  ♛ ॐ,  ♛ ॐ");
                    unicode_text.text(" ♣▄♠░ ");
                    unicode_text.text("♣♠░:D ︻デ── ");
                });

            run_loop
        },
    );
    Ok(())
}
