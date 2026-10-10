#pragma once

#include <string>
#include <utility>
#include <vector>

// Clipboard contents as (MIME type, data) pairs.
using SystemClipboardEntries = std::vector<std::pair<std::string, std::string>>;

// Reads the macOS pasteboard's text, HTML and PNG contents.
SystemClipboardEntries photon_system_clipboard_read();
// Replaces the macOS pasteboard's contents.
void photon_system_clipboard_write(SystemClipboardEntries const& entries);
