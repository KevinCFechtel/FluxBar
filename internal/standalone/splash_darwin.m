//go:build darwin

#import <Cocoa/Cocoa.h>
#import <dispatch/dispatch.h>
#import "localization_darwin.h"

static NSPanel *fluxbarSplashPanel;

void fluxbar_show_startup_splash(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [fluxbarSplashPanel close];

        NSRect frame = NSMakeRect(0, 0, 390, 170);
        NSPanel *panel = [[NSPanel alloc]
            initWithContentRect:frame
                      styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                        backing:NSBackingStoreBuffered
                          defer:NO];
        panel.opaque = NO;
        panel.backgroundColor = NSColor.clearColor;
        panel.hasShadow = YES;
        panel.level = NSFloatingWindowLevel;
        panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                   NSWindowCollectionBehaviorTransient |
                                   NSWindowCollectionBehaviorIgnoresCycle;

        NSVisualEffectView *background = [[NSVisualEffectView alloc] initWithFrame:frame];
        background.material = NSVisualEffectMaterialPopover;
        background.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        background.state = NSVisualEffectStateActive;
        background.wantsLayer = YES;
        background.layer.cornerRadius = 18;
        background.layer.masksToBounds = YES;
        panel.contentView = background;

        NSImageView *iconView = [[NSImageView alloc] initWithFrame:NSMakeRect(28, 43, 84, 84)];
        iconView.image = NSApp.applicationIconImage;
        iconView.imageScaling = NSImageScaleProportionallyUpOrDown;
        [background addSubview:iconView];

        NSTextField *title = [NSTextField labelWithString:@"FluxNews"];
        title.frame = NSMakeRect(136, 91, 226, 34);
        title.font = [NSFont systemFontOfSize:26 weight:NSFontWeightSemibold];
        [background addSubview:title];

        NSTextField *message = [NSTextField wrappingLabelWithString:FluxBarLocalized(
            @"splash.message",
            @"FluxNews is now running in the menu bar and loading your unread articles."
        )];
        message.frame = NSMakeRect(136, 43, 226, 44);
        message.font = [NSFont systemFontOfSize:13];
        message.textColor = NSColor.secondaryLabelColor;
        [background addSubview:message];

        [panel center];
        panel.alphaValue = 0;
        [panel orderFrontRegardless];
        [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
            context.duration = 0.18;
            panel.animator.alphaValue = 1;
        } completionHandler:nil];

        fluxbarSplashPanel = panel;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(2.5 * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{
            if (fluxbarSplashPanel != panel) {
                return;
            }
            [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
                context.duration = 0.3;
                panel.animator.alphaValue = 0;
            } completionHandler:^{
                [panel close];
                if (fluxbarSplashPanel == panel) {
                    fluxbarSplashPanel = nil;
                }
            }];
        });
    });
}
