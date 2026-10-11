#import <AppKit/AppKit.h>

#include "PhotonEmbedderBridge.h"

// Only an app bundle can be the default browser: macOS opens links with
// bundles that declare the http and https schemes.
static NSURL* photon_app_bundle_url()
{
    auto* bundle = [NSBundle mainBundle];
    if (bundle.bundleIdentifier == nil || ![bundle.bundleURL.pathExtension isEqualToString:@"app"])
        return nil;
    return bundle.bundleURL;
}

extern "C" bool photon_default_browser_available(void)
{
    return photon_app_bundle_url() != nil;
}

extern "C" bool photon_default_browser_is_current(void)
{
    auto* app = photon_app_bundle_url();
    if (app == nil)
        return false;
    auto* handler = [[NSWorkspace sharedWorkspace] URLForApplicationToOpenURL:[NSURL URLWithString:@"https://example.com"]];
    return handler != nil
        && [handler.URLByStandardizingPath.URLByResolvingSymlinksInPath
            isEqual:app.URLByStandardizingPath.URLByResolvingSymlinksInPath];
}

extern "C" void photon_default_browser_request(void)
{
    auto* app = photon_app_bundle_url();
    if (app == nil)
        return;
    // macOS asks the person to confirm the change.
    for (NSString* scheme in @[ @"http", @"https" ]) {
        [[NSWorkspace sharedWorkspace] setDefaultApplicationAtURL:app
                                             toOpenURLsWithScheme:scheme
                                                completionHandler:nil];
    }
}
