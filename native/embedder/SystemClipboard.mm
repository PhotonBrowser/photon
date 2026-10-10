#import <AppKit/AppKit.h>

#include "SystemClipboard.h"

#include <array>

namespace {

struct PasteboardType {
    char const* mime_type;
    NSPasteboardType type;
};

// The representations Photon exchanges with other apps, most specific first.
std::array<PasteboardType, 4> const& pasteboard_types()
{
    static std::array<PasteboardType, 4> const types {
        PasteboardType { "image/png", NSPasteboardTypePNG },
        PasteboardType { "text/html", NSPasteboardTypeHTML },
        PasteboardType { "text/uri-list", NSPasteboardTypeURL },
        PasteboardType { "text/plain", NSPasteboardTypeString },
    };
    return types;
}

}

SystemClipboardEntries photon_system_clipboard_read()
{
    SystemClipboardEntries entries;
    @autoreleasepool {
        auto* pasteboard = [NSPasteboard generalPasteboard];
        for (auto const& type : pasteboard_types()) {
            NSData* data = [pasteboard dataForType:type.type];
            if (data)
                entries.emplace_back(type.mime_type,
                    std::string(static_cast<char const*>(data.bytes), data.length));
        }
    }
    return entries;
}

void photon_system_clipboard_write(SystemClipboardEntries const& entries)
{
    @autoreleasepool {
        auto* pasteboard = [NSPasteboard generalPasteboard];
        [pasteboard clearContents];
        for (auto const& [mime_type, data] : entries) {
            for (auto const& type : pasteboard_types()) {
                if (mime_type != type.mime_type)
                    continue;
                [pasteboard setData:[NSData dataWithBytes:data.data() length:data.size()]
                            forType:type.type];
            }
        }
    }
}
