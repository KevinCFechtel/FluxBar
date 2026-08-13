//go:build darwin

#import <Cocoa/Cocoa.h>
#import <dispatch/dispatch.h>
#import <objc/runtime.h>
#import <stdbool.h>

static const NSTimeInterval FluxBarHoverDelay = 0.5;
static const NSUInteger FluxBarMaximumImageBytes = 10 * 1024 * 1024;

@interface FluxBarArticlePreview : NSObject
@property(nonatomic, copy) NSString *title;
@property(nonatomic, copy) NSString *feed;
@property(nonatomic, copy) NSString *preview;
@property(nonatomic, strong) NSURL *imageURL;
@property(nonatomic, strong) NSData *fallbackIcon;
@end

@implementation FluxBarArticlePreview
@end

static NSMenu *fluxbarMenu;
static NSMutableDictionary<NSNumber *, FluxBarArticlePreview *> *fluxbarPreviews;
static NSPanel *fluxbarHoverPanel;
static NSURLSessionDataTask *fluxbarHoverImageTask;
static id fluxbarTrackingStartObserver;
static id fluxbarTrackingEndObserver;
static NSUInteger fluxbarHoverGeneration;
static bool fluxbarMenuTracking;
static IMP fluxbarOriginalWillHighlight;

static void fluxbar_run_main_sync(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_sync(dispatch_get_main_queue(), block);
    }
}

static NSString *fluxbar_string(const char *value) {
    if (value == NULL) {
        return @"";
    }
    NSString *result = [NSString stringWithUTF8String:value];
    return result != nil ? result : @"";
}

static id fluxbar_delegate_value(NSString *key) {
    id delegate = NSApp.delegate;
    if (delegate == nil) {
        return nil;
    }
    @try {
        return [delegate valueForKey:key];
    } @catch (NSException *exception) {
        return nil;
    }
}

static NSMenu *fluxbar_systray_menu(void) {
    id menu = fluxbar_delegate_value(@"menu");
    return [menu isKindOfClass:[NSMenu class]] ? menu : nil;
}

static NSTextField *fluxbar_label(NSString *text, NSFont *font, NSColor *color) {
    NSTextField *label = [NSTextField wrappingLabelWithString:text];
    label.font = font;
    label.textColor = color;
    label.maximumNumberOfLines = 0;
    label.lineBreakMode = NSLineBreakByWordWrapping;
    return label;
}

static NSURL *fluxbar_http_url(NSString *value) {
    NSURL *url = [NSURL URLWithString:value];
    NSString *scheme = url.scheme.lowercaseString;
    if (![scheme isEqualToString:@"http"] && ![scheme isEqualToString:@"https"]) {
        return nil;
    }
    return url;
}

static void fluxbar_close_hover_panel(void) {
    [fluxbarHoverImageTask cancel];
    fluxbarHoverImageTask = nil;
    [fluxbarHoverPanel orderOut:nil];
    fluxbarHoverPanel = nil;
}

static NSScreen *fluxbar_screen_for_point(NSPoint point) {
    for (NSScreen *screen in NSScreen.screens) {
        if (NSPointInRect(point, screen.frame)) {
            return screen;
        }
    }
    return NSScreen.mainScreen;
}

static void fluxbar_position_panel(NSPanel *panel) {
    NSPoint mouse = NSEvent.mouseLocation;
    NSScreen *screen = fluxbar_screen_for_point(mouse);
    NSRect visible = screen != nil ? screen.visibleFrame : NSMakeRect(0, 0, 1440, 900);
    NSSize size = panel.frame.size;
    CGFloat gap = 24;
    CGFloat x = mouse.x - size.width - gap;
    if (x < NSMinX(visible)) {
        x = mouse.x + gap;
    }
    x = MIN(MAX(x, NSMinX(visible)), NSMaxX(visible) - size.width);
    CGFloat y = mouse.y - size.height / 2;
    y = MIN(MAX(y, NSMinY(visible)), NSMaxY(visible) - size.height);
    [panel setFrameOrigin:NSMakePoint(x, y)];
}

static void fluxbar_load_hover_image(
    NSURL *url,
    NSImageView *imageView,
    NSPanel *panel,
    NSUInteger generation
) {
    if (url == nil) {
        return;
    }
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:url
                                                          cachePolicy:NSURLRequestReturnCacheDataElseLoad
                                                      timeoutInterval:12.0];
    [request setValue:@"image/avif,image/webp,image/*,*/*;q=0.8" forHTTPHeaderField:@"Accept"];
    fluxbarHoverImageTask = [[NSURLSession sharedSession]
        dataTaskWithRequest:request
          completionHandler:^(NSData *data, NSURLResponse *response, NSError *error) {
            if (error != nil || data.length == 0 || data.length > FluxBarMaximumImageBytes) {
                return;
            }
            NSString *mimeType = response.MIMEType.lowercaseString;
            if (mimeType.length > 0 && ![mimeType hasPrefix:@"image/"]) {
                return;
            }
            NSImage *image = [[NSImage alloc] initWithData:data];
            if (image == nil) {
                return;
            }
            dispatch_async(dispatch_get_main_queue(), ^{
                if (fluxbarHoverGeneration == generation && fluxbarHoverPanel == panel) {
                    imageView.imageScaling = NSImageScaleProportionallyUpOrDown;
                    imageView.image = image;
                }
            });
          }];
    [fluxbarHoverImageTask resume];
}

static void fluxbar_show_hover_panel(FluxBarArticlePreview *article, NSUInteger generation) {
    fluxbar_close_hover_panel();

    bool hasImage = article.imageURL != nil;
    CGFloat width = 440;
    CGFloat height = hasImage ? 510 : 300;

    NSVisualEffectView *root = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
    root.material = NSVisualEffectMaterialPopover;
    root.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    root.state = NSVisualEffectStateActive;
    root.wantsLayer = YES;
    root.layer.cornerRadius = 12;
    root.layer.masksToBounds = YES;

    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = 10;
    stack.edgeInsets = NSEdgeInsetsMake(16, 16, 16, 16);
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [root addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:root.leadingAnchor],
        [stack.trailingAnchor constraintEqualToAnchor:root.trailingAnchor],
        [stack.topAnchor constraintEqualToAnchor:root.topAnchor],
        [stack.bottomAnchor constraintEqualToAnchor:root.bottomAnchor]
    ]];

    NSImageView *imageView = nil;
    if (hasImage) {
        imageView = [[NSImageView alloc] init];
        imageView.imageScaling = NSImageScaleProportionallyDown;
        imageView.imageAlignment = NSImageAlignCenter;
        imageView.wantsLayer = YES;
        imageView.layer.cornerRadius = 8;
        imageView.layer.masksToBounds = YES;
        imageView.layer.backgroundColor = NSColor.controlBackgroundColor.CGColor;
        if (article.fallbackIcon.length > 0) {
            imageView.image = [[NSImage alloc] initWithData:article.fallbackIcon];
        }
        imageView.translatesAutoresizingMaskIntoConstraints = NO;
        [imageView.heightAnchor constraintEqualToConstant:180].active = YES;
        [imageView.widthAnchor constraintEqualToConstant:408].active = YES;
        [stack addArrangedSubview:imageView];
    }

    NSTextField *titleLabel = fluxbar_label(
        article.title,
        [NSFont boldSystemFontOfSize:15],
        NSColor.labelColor
    );
    [titleLabel.widthAnchor constraintEqualToConstant:408].active = YES;
    [stack addArrangedSubview:titleLabel];
    if (article.feed.length > 0) {
        NSTextField *feedLabel = fluxbar_label(
            article.feed,
            [NSFont systemFontOfSize:12],
            NSColor.secondaryLabelColor
        );
        [feedLabel.widthAnchor constraintEqualToConstant:408].active = YES;
        [stack addArrangedSubview:feedLabel];
    }

    NSScrollView *scrollView = [[NSScrollView alloc] init];
    scrollView.hasVerticalScroller = YES;
    scrollView.drawsBackground = NO;
    scrollView.borderType = NSNoBorder;
    scrollView.translatesAutoresizingMaskIntoConstraints = NO;
    [scrollView.heightAnchor constraintEqualToConstant:(hasImage ? 210 : 190)].active = YES;
    [scrollView.widthAnchor constraintEqualToConstant:408].active = YES;

    NSTextView *textView = [[NSTextView alloc] initWithFrame:NSMakeRect(0, 0, 408, 190)];
    textView.string = article.preview.length > 0 ? article.preview : @"Keine Textvorschau verfügbar.";
    textView.font = [NSFont systemFontOfSize:13];
    textView.textColor = NSColor.labelColor;
    textView.editable = NO;
    textView.selectable = NO;
    textView.drawsBackground = NO;
    textView.textContainerInset = NSMakeSize(0, 4);
    textView.verticallyResizable = YES;
    textView.horizontallyResizable = NO;
    textView.autoresizingMask = NSViewWidthSizable;
    textView.textContainer.widthTracksTextView = YES;
    scrollView.documentView = textView;
    [stack addArrangedSubview:scrollView];

    NSPanel *panel = [[NSPanel alloc]
        initWithContentRect:NSMakeRect(0, 0, width, height)
                  styleMask:(NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    panel.contentView = root;
    panel.opaque = NO;
    panel.backgroundColor = NSColor.clearColor;
    panel.hasShadow = YES;
    panel.level = NSPopUpMenuWindowLevel;
    panel.ignoresMouseEvents = YES;
    panel.hidesOnDeactivate = NO;
    panel.collectionBehavior = NSWindowCollectionBehaviorTransient | NSWindowCollectionBehaviorMoveToActiveSpace;
    fluxbarHoverPanel = panel;
    fluxbar_position_panel(panel);
    [panel orderFrontRegardless];

    if (imageView != nil) {
        fluxbar_load_hover_image(article.imageURL, imageView, panel, generation);
    }
}

static void fluxbar_highlight_article(NSMenuItem *item) {
    fluxbarHoverGeneration++;
    NSUInteger generation = fluxbarHoverGeneration;
    fluxbar_close_hover_panel();

    NSNumber *menuID = [item.representedObject isKindOfClass:[NSNumber class]]
        ? item.representedObject
        : nil;
    FluxBarArticlePreview *article = menuID != nil ? fluxbarPreviews[menuID] : nil;
    if (article == nil) {
        return;
    }

    dispatch_after(
        dispatch_time(DISPATCH_TIME_NOW, (int64_t)(FluxBarHoverDelay * NSEC_PER_SEC)),
        dispatch_get_main_queue(),
        ^{
            if (fluxbarMenuTracking && fluxbarHoverGeneration == generation) {
                fluxbar_show_hover_panel(article, generation);
            }
        }
    );
}

static void fluxbar_menu_will_highlight(id owner, SEL selector, NSMenu *menu, NSMenuItem *item) {
    if (fluxbarOriginalWillHighlight != NULL) {
        ((void (*)(id, SEL, NSMenu *, NSMenuItem *))fluxbarOriginalWillHighlight)(owner, selector, menu, item);
    }
    if (menu == fluxbarMenu) {
        fluxbar_highlight_article(item);
    }
}

bool fluxbar_initialize_article_hover(void) {
    __block bool initialized = false;
    fluxbar_run_main_sync(^{
        if (fluxbarMenu != nil) {
            initialized = true;
            return;
        }
        fluxbarMenu = fluxbar_systray_menu();
        if (fluxbarMenu == nil) {
            return;
        }
        fluxbarPreviews = [[NSMutableDictionary alloc] init];

        Class delegateClass = [NSApp.delegate class];
        SEL selector = @selector(menu:willHighlightItem:);
        Method existing = class_getInstanceMethod(delegateClass, selector);
        if (existing != NULL) {
            fluxbarOriginalWillHighlight = method_getImplementation(existing);
            method_setImplementation(existing, (IMP)fluxbar_menu_will_highlight);
        } else {
            class_addMethod(delegateClass, selector, (IMP)fluxbar_menu_will_highlight, "v@:@@");
        }

        NSNotificationCenter *notifications = NSNotificationCenter.defaultCenter;
        fluxbarTrackingStartObserver = [notifications
            addObserverForName:NSMenuDidBeginTrackingNotification
                        object:fluxbarMenu
                         queue:NSOperationQueue.mainQueue
                    usingBlock:^(NSNotification *notification) {
                        fluxbarMenuTracking = true;
                    }];
        fluxbarTrackingEndObserver = [notifications
            addObserverForName:NSMenuDidEndTrackingNotification
                        object:fluxbarMenu
                         queue:NSOperationQueue.mainQueue
                    usingBlock:^(NSNotification *notification) {
                        fluxbarMenuTracking = false;
                        fluxbarHoverGeneration++;
                        fluxbar_close_hover_panel();
                    }];
        initialized = true;
    });
    return initialized;
}

void fluxbar_reset_article_hover(void) {
    fluxbar_run_main_sync(^{
        fluxbarHoverGeneration++;
        [fluxbarPreviews removeAllObjects];
        fluxbar_close_hover_panel();
    });
}

void fluxbar_register_article_hover(
    const char *titleValue,
    const char *feedValue,
    const char *previewValue,
    const char *imageURLValue,
    const unsigned char *fallbackIconBytes,
    int fallbackIconLength
) {
    FluxBarArticlePreview *article = [[FluxBarArticlePreview alloc] init];
    article.title = fluxbar_string(titleValue);
    article.feed = fluxbar_string(feedValue);
    article.preview = fluxbar_string(previewValue);
    article.imageURL = fluxbar_http_url(fluxbar_string(imageURLValue));
    article.fallbackIcon = fallbackIconLength > 0
        ? [NSData dataWithBytes:fallbackIconBytes length:(NSUInteger)fallbackIconLength]
        : nil;

    fluxbar_run_main_sync(^{
        NSMenuItem *item = fluxbarMenu.itemArray.lastObject;
        NSNumber *menuID = [item.representedObject isKindOfClass:[NSNumber class]]
            ? item.representedObject
            : nil;
        if (menuID != nil) {
            fluxbarPreviews[menuID] = article;
        }
    });
}

void fluxbar_close_article_hover(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        fluxbarMenuTracking = false;
        fluxbarHoverGeneration++;
        fluxbar_close_hover_panel();
        [fluxbarPreviews removeAllObjects];
        NSNotificationCenter *notifications = NSNotificationCenter.defaultCenter;
        if (fluxbarTrackingStartObserver != nil) {
            [notifications removeObserver:fluxbarTrackingStartObserver];
        }
        if (fluxbarTrackingEndObserver != nil) {
            [notifications removeObserver:fluxbarTrackingEndObserver];
        }
        fluxbarTrackingStartObserver = nil;
        fluxbarTrackingEndObserver = nil;
        fluxbarMenu = nil;
    });
}
