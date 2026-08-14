//go:build darwin

#import "localization_darwin.h"
#import <stdlib.h>
#import <string.h>

NSString *FluxBarLocalized(NSString *key, NSString *fallback) {
    const char *keyValue = (key ?: @"").UTF8String;
    const char *fallbackValue = (fallback ?: @"").UTF8String;
    char *localizedValue = FluxBarCopyLocalizedString(
        keyValue != NULL ? keyValue : "",
        fallbackValue != NULL ? fallbackValue : ""
    );
    if (localizedValue == NULL) {
        return fallback ?: @"";
    }
    NSString *localized = [NSString stringWithUTF8String:localizedValue];
    free(localizedValue);
    return localized ?: fallback ?: @"";
}
