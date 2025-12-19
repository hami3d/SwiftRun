# SwiftRun Modular Architecture 🏗️

This document outlines the organization of the SwiftRun codebase. The project is structured to separate concerns between System logic, UI rendering, and Application state, ensuring high maintainability and clarity.

## 📁 Directory Structure

### `src/`
*   **`main.rs`**: Application entry point. Handles window class registration, and the high-level orchestration of the main message loop.
*   **`config.rs`**: The **Design System & Config**. Contains all UI constants, layout dimensions, shared colors, and static configuration strings. Modify this for layout or aesthetic changes.
*   **`animations.rs`**: Math engine. Contains easing functions and interpolation logic for smooth Fluent UI transitions.

### `src/ui/` (The View Layer)
*   **`mod.rs`**: Central UI utility module. Handles DPI scaling, Windows Accent Color detection, and the "Acrylic" background effect logic.
*   **`resources.rs`**: The **Global State Store**. Centralized owner of Direct2D/DirectWrite/WIC factories, window handles (`HWND`), the shared `INPUT_BUFFER`, and application-specific message constants.
*   **`main_win.rs`**: Implements the primary window logic, including its specific `wndproc`, rendering commands, and user input handling.
*   **`dropdown.rs`**: Logic for the command history suggestions menu, including its own rendering and animation state.
*   **`tooltip.rs`**: Lightweight notification system for feedback (e.g., "Command Not Found").
*   **`dialog.rs`**: Custom Fluent Design message dialogs used for installation feedback and error reporting.

### `src/system/` (The OS Bridge)
*   **`executor.rs`**: The **Command Engine**. Handles command parsing, admin elevation detection, URL handling, and asynchronous process spawning.
*   **`registry.rs`**: Manages installation state, including autostart and the "DisabledHotkeys" registry hijacking used to take over Win+R.
*   **`hotkeys.rs`**: Encapsulates Windows Global Hotkey registration and cleanup logic.
*   **`explorer.rs`**: Provides logic for restarting `explorer.exe` to apply low-level shell changes.

### `src/data/` (Persistence)
*   **`history.rs`**: Logic for loading, saving, and managing the persistent command history file, including the history cycling engine.

---

## 🛠️ Development Guidelines

1.  **Centralized Constants**: Constants should ALWAYS be placed in `config.rs`. Avoid hardcoding magic numbers in rendering or logic files.
2.  **Modular Message Handling**: Logic for specific windows should stay within their respective `ui/` files. The `main.rs` loop should remain high-level.
3.  **Global Safe Access**: Use the centralized handles and factories in `ui/resources.rs` rather than declaring local statics to avoid duplication and ownership confusion.
4.  **Async Execution**: Heavy operations (like launching external processes) must be moved to background threads (typically in `executor.rs`) to prevent UI hanging.
5.  **Fluent Design Compliance**: Use the utilities in `ui/mod.rs` and the easing functions in `animations.rs` to maintain the modern Windows aesthetic.
