# EvertyUI

<div align="center">

**Lightweight, deterministic, cross-platform UI toolkit in pure Rust.**  
*Легковесный, детерминированный кроссплатформенный UI-фреймворк на чистом Rust.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/tests-146%20passed-brightgreen.svg)]()
[![Donate](https://img.shields.io/badge/Support-Boosty-red.svg)](https://boosty.to/vaali/donate)

[English](#english) • [Русский](#русский) • [Products & Ecosystem](#products-powered-by-evertyui) • [Donation](#поддержать-проект--support)

---

</div>

<a name="english"></a>
## English

**EvertyUI** is a lightweight, zero-overhead desktop UI foundation engineered specifically for mission-critical client applications. It guarantees `#![forbid(unsafe_code)]`, zero GPU driver bloat via software-rendered presentation (`softbuffer`), deterministic pixel snapping across mixed-DPI monitors, and zero idle CPU usage.

### Key Capabilities

* **0% CPU Idle App Runner**: Built on `winit` 0.30 and `softbuffer` 0.4. Fully reactive event loop that sleeps until an OS event or background worker signal (`EventLoopProxy`) wakes it up.
* **Deterministic Layout & DPI Engine**: Logical and physical pixel boundaries are strictly separated. Grid snapping happens once in device coordinates to prevent subpixel seams and blurring.
* **Secure Text & Password Editor**:
  * Prevents secret leakage: Undo/Redo history is completely bypassed in password mode.
  * Safe `Debug` implementation redacting confidential input to `"[REDACTED]"`.
  * Copy/cut commands (`Ctrl+C`, `Ctrl+X`) are securely absorbed in password mode to avoid leakage to the clipboard or remote host terminal sessions.
* **Full IME Support**: First-class handling of input method preedit compositions and clean Enter confirmations without premature form submissions.
* **Comprehensive UI Kit**:
  * Buttons, IconButtons with SVG rasterization (`resvg`).
  * Checkboxes (`CheckState` with indeterminate support) and smooth Switches.
  * TabBar with selection, keyboard navigation, and close badges.
  * Modal Dialogs with focus trapping and backdrop click isolation.
  * Interactive Scrollbars and Keyed Virtual Lists (`KeyedSelection` invariant to sorting/filtering).
* **Robust Window Lifecycle**: Integrated `WindowCloseHandler` coordinating background tasks (OCR, exports) and unsaved changes confirmation dialogs.

---

<a name="русский"></a>
## Русский

**EvertyUI** — это компактный, высоконадёжный UI-фреймворк для десктопных приложений на Rust. Он ориентирован на высокую отзывчивость, полную изоляцию от тяжёлых GPU-стеков (прямой рендеринг через `softbuffer`), безопасность данных и отсутствие фоновой нагрузки на процессор в состоянии покоя.

### Основные возможности

* **Рабочий цикл с 0% CPU в простое**: Работает на связке `winit` 0.30 и `softbuffer` 0.4. Цикл событий засыпает без поллинга и мгновенно просыпается по событиям ОС или сигналам фоновых потоков (`EventLoopProxy`).
* **Детерминированная геометрия и DPI**: Логические и физические координаты строго разграничены. Привязка к пиксельной сетке выполняется один раз в пространстве устройства, устраняя швы и мыло при масштабировании.
* **Защищённый ввод паролей**:
  * История Undo/Redo полностью отключается в парольном режиме — секреты не оседают в буферах отмены.
  * Кастомный вывод `Debug` маскирует введённые данные в `"[REDACTED]"`.
  * Команды копирования и вырезания (`Ctrl+C`, `Ctrl+X`) поглощаются внутри поля, предотвращая утечку пароля в буфер обмена или удалённый терминал.
* **Поддержка IME**: Корректная обработка композиции ввода и подтверждения по `Enter` без ложной отправки диалогов.
* **Набор компонентов**:
  * Кнопки и иконки с векторизацией SVG (`resvg`).
  * Чекбоксы (с поддержкой неопределённого состояния) и тумблеры (Switch).
  * Вкладки (TabBar) с управлением фокусом и закрытием.
  * Модальные окна с изоляцией фокуса и перехватом кликов по подложке.
  * Виртуальные списки с идентификацией элементов (`ItemId`, `KeyedSelection`), сохраняющие выделение при фильтрации и сортировке.
* **Жизненный цикл окна**: Встроенный `WindowCloseHandler` для координации фоновых задач и подтверждения несохранённых данных при закрытии.

---

## Products Powered by EvertyUI / Проекты на EvertyUI

EvertyUI служит единым UI-ядром для экосистемы прикладных продуктов:

* 🌐 [**EvertyDesk Next** (desk.everty.ru)](https://desk.everty.ru) — удалённый доступ и управление рабочими столами нового поколения.
* 📄 [**atletPDF** (atletpdf.ru)](https://atletpdf.ru) — быстрый десктопный инструмент для работы и редактирования PDF-документов.
* 🔗 [**EvertyMultiLink**](https://github.com/vaalimusic/EvertyMultiLink) — высокопроизводительный инструмент маршрутизации и агрегации сетевых каналов.
* 💻 [**EvertySSH**](https://github.com/vaalimusic/EvertySSH) — современный и безопасный SSH-клиент с управлением сессиями.

---

## Поддержать проект / Support

Если вам нравится проект и вы хотите поддержать его дальнейшее развитие:

* 💖 **[Поддержать на Boosty (Donate via Boosty)](https://boosty.to/vaali/donate)**

---

## Quick Start / Быстрый старт

```rust
use everty_ui::{
    AppHandler, AppRunner, AppWindowContext, Button, ButtonState, HitRegion,
    LogicalRect, PaintBuilder, PaintLayer, TextSystem, Theme, WidgetId, WindowConfig,
};

struct DemoApp {
    button: Button,
}

impl AppHandler for DemoApp {
    type UserEvent = ();

    fn paint(
        &mut self,
        builder: &mut PaintBuilder,
        text: &mut TextSystem,
        theme: &Theme,
        _viewport: LogicalRect,
    ) -> Vec<HitRegion> {
        let mut regions = Vec::new();
        let rect = LogicalRect::from_xywh(24.0, 24.0, 160.0, 36.0);
        let state = ButtonState {
            hovered: false,
            pressed: false,
            focused: false,
            enabled: true,
        };
        self.button.paint(rect, state, text, *theme, builder);
        regions.push(self.button.hit_region(rect, PaintLayer::Content, 1));
        regions
    }
}

fn main() -> Result<(), String> {
    let config = WindowConfig {
        title: "EvertyUI Application".into(),
        width: 600.0,
        height: 400.0,
        ..Default::default()
    };
    let app = DemoApp {
        button: Button::new(WidgetId(1), "Everty Button"),
    };
    AppRunner::new(config, app).run()
}
```

---

## License / Лицензия

This project is licensed under the [MIT License](LICENSE).
