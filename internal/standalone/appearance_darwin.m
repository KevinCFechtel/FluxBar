//go:build darwin

#import <Cocoa/Cocoa.h>
#import <dispatch/dispatch.h>
#import <stdbool.h>
#import <stdint.h>

extern void fluxbar_appearance_changed(uintptr_t context, bool dark);

static void *FluxBarAppearanceObservationContext = &FluxBarAppearanceObservationContext;

@interface FluxBarAppearanceObserver : NSObject
@property(nonatomic, assign) uintptr_t callbackContext;
@end

static FluxBarAppearanceObserver *fluxbarAppearanceObserver;

static void fluxbar_run_appearance_main_sync(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_sync(dispatch_get_main_queue(), block);
    }
}

static bool fluxbar_read_dark_appearance(void) {
    NSAppearance *appearance = NSApp.effectiveAppearance;
    if (appearance == nil) {
        NSString *style = [[NSUserDefaults standardUserDefaults] stringForKey:@"AppleInterfaceStyle"];
        return [style caseInsensitiveCompare:@"Dark"] == NSOrderedSame;
    }
    NSAppearanceName match = [appearance bestMatchFromAppearancesWithNames:@[
        NSAppearanceNameAqua,
        NSAppearanceNameDarkAqua
    ]];
    return [match isEqualToString:NSAppearanceNameDarkAqua];
}

@implementation FluxBarAppearanceObserver

- (void)observeValueForKeyPath:(NSString *)keyPath
                      ofObject:(id)object
                        change:(NSDictionary<NSKeyValueChangeKey, id> *)change
                       context:(void *)context {
    if (context == FluxBarAppearanceObservationContext) {
        fluxbar_appearance_changed(self.callbackContext, fluxbar_read_dark_appearance());
        return;
    }
    [super observeValueForKeyPath:keyPath ofObject:object change:change context:context];
}

@end

bool fluxbar_is_dark_appearance(void) {
    if ([NSThread isMainThread]) {
        return fluxbar_read_dark_appearance();
    }

    __block bool dark = false;
    dispatch_sync(dispatch_get_main_queue(), ^{
        dark = fluxbar_read_dark_appearance();
    });
    return dark;
}

bool fluxbar_start_appearance_observation(uintptr_t context) {
    __block bool started = false;
    fluxbar_run_appearance_main_sync(^{
        if (fluxbarAppearanceObserver != nil) {
            started = true;
            return;
        }

        FluxBarAppearanceObserver *observer = [[FluxBarAppearanceObserver alloc] init];
        observer.callbackContext = context;
        @try {
            [NSApp addObserver:observer
                    forKeyPath:@"effectiveAppearance"
                       options:NSKeyValueObservingOptionNew
                       context:FluxBarAppearanceObservationContext];
            fluxbarAppearanceObserver = observer;
            started = true;
        } @catch (NSException *exception) {
            fluxbarAppearanceObserver = nil;
        }
    });
    return started;
}

void fluxbar_stop_appearance_observation(void) {
    fluxbar_run_appearance_main_sync(^{
        if (fluxbarAppearanceObserver == nil) {
            return;
        }
        @try {
            [NSApp removeObserver:fluxbarAppearanceObserver
                       forKeyPath:@"effectiveAppearance"
                          context:FluxBarAppearanceObservationContext];
        } @catch (NSException *exception) {
            // The app is shutting down; there is nothing useful to recover here.
        }
        fluxbarAppearanceObserver = nil;
    });
}
