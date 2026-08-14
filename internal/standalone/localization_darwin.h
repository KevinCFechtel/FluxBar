#ifndef FLUXBAR_LOCALIZATION_DARWIN_H
#define FLUXBAR_LOCALIZATION_DARWIN_H

#ifdef __OBJC__
#import <Foundation/Foundation.h>

NSString *FluxBarLocalized(NSString *key, NSString *fallback);
#endif

char *FluxBarCopyLocalizedString(const char *key, const char *fallback);

#endif
