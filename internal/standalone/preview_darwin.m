//go:build darwin

#import <Cocoa/Cocoa.h>
#import <dispatch/dispatch.h>
#import <objc/runtime.h>
#import "localization_darwin.h"
#import <stdbool.h>

static const NSTimeInterval FluxBarHoverDelay = 0.5;
static const NSUInteger FluxBarMaximumImageBytes = 10 * 1024 * 1024;
static const CGFloat FluxBarPreviewHorizontalPadding = 16;
static const CGFloat FluxBarPreviewVerticalPadding = 16;
static const CGFloat FluxBarPreviewSpacing = 10;

typedef struct {
    CGFloat width;
    CGFloat height;
    CGFloat contentWidth;
    CGFloat imageHeight;
    CGFloat titleHeight;
    CGFloat feedHeight;
    CGFloat feedWidth;
    CGFloat dateWidth;
    CGFloat previewHeight;
    bool previewScrolls;
} FluxBarPreviewLayout;

@interface FluxBarArticlePreview : NSObject
@property(nonatomic, copy) NSString *title;
@property(nonatomic, copy) NSString *feed;
@property(nonatomic, copy) NSString *preview;
@property(nonatomic, strong) NSDate *publishedAt;
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

static NSRect fluxbar_visible_frame_for_point(NSPoint point) {
    NSScreen *screen = fluxbar_screen_for_point(point);
    return screen != nil ? screen.visibleFrame : NSMakeRect(0, 0, 1440, 900);
}

static void fluxbar_position_panel(NSPanel *panel) {
    NSPoint mouse = NSEvent.mouseLocation;
    NSRect visible = fluxbar_visible_frame_for_point(mouse);
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

static CGFloat fluxbar_measured_text_height(NSString *text, NSFont *font, CGFloat width) {
    if (text.length == 0 || width <= 0) {
        return 0;
    }
    NSRect bounds = [text boundingRectWithSize:NSMakeSize(width, CGFLOAT_MAX)
                                       options:NSStringDrawingUsesLineFragmentOrigin |
                                               NSStringDrawingUsesFontLeading
                                    attributes:@{NSFontAttributeName: font}];
    return ceil(bounds.size.height);
}

static NSString *fluxbar_published_date(NSDate *date) {
    if (date == nil) {
        return @"";
    }
    static NSDateFormatter *formatter;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        formatter = [[NSDateFormatter alloc] init];
        formatter.dateStyle = NSDateFormatterShortStyle;
        formatter.timeStyle = NSDateFormatterShortStyle;
        formatter.doesRelativeDateFormatting = YES;
    });
    return [formatter stringFromDate:date];
}

static FluxBarPreviewLayout fluxbar_preview_layout(
    FluxBarArticlePreview *article,
    bool hasImage,
    NSRect visibleFrame
) {
    NSFont *titleFont = [NSFont boldSystemFontOfSize:15];
    NSFont *feedFont = [NSFont systemFontOfSize:12];
    NSFont *previewFont = [NSFont systemFontOfSize:13];
    NSString *publishedDate = fluxbar_published_date(article.publishedAt);
    bool hasMetadata = article.feed.length > 0 || publishedDate.length > 0;
    NSString *previewText = article.preview.length > 0
        ? article.preview
        : FluxBarLocalized(@"preview.no_text", @"No text preview available.");

    CGFloat maximumWidth = MIN(500, MAX(260, visibleFrame.size.width - 32));
    CGFloat minimumWidth = MIN(hasImage ? 360 : 300, maximumWidth);
    CGFloat maximumHeight = MIN(hasImage ? 520 : 340, MAX(180, visibleFrame.size.height - 32));

    FluxBarPreviewLayout selected = {0};
    for (CGFloat width = minimumWidth; width <= maximumWidth; width += 20) {
        CGFloat contentWidth = width - 2 * FluxBarPreviewHorizontalPadding;
        CGFloat imageHeight = hasImage ? MIN(180, floor(contentWidth * 0.46)) : 0;
        CGFloat titleHeight = MIN(
            fluxbar_measured_text_height(article.title, titleFont, contentWidth),
            ceil(titleFont.pointSize * 1.25 * 3)
        );
        CGFloat measuredDateWidth = publishedDate.length > 0
            ? ceil([publishedDate sizeWithAttributes:@{NSFontAttributeName: feedFont}].width)
            : 0;
        CGFloat dateWidth = MIN(measuredDateWidth, floor(contentWidth * 0.46));
        CGFloat metadataGap = article.feed.length > 0 && publishedDate.length > 0 ? 12 : 0;
        CGFloat feedWidth = article.feed.length > 0
            ? MAX(0, contentWidth - dateWidth - metadataGap)
            : 0;
        CGFloat measuredFeedHeight = article.feed.length > 0
            ? MIN(
                fluxbar_measured_text_height(article.feed, feedFont, feedWidth),
                ceil(feedFont.pointSize * 1.25 * 2)
            )
            : 0;
        CGFloat dateHeight = publishedDate.length > 0
            ? fluxbar_measured_text_height(publishedDate, feedFont, dateWidth)
            : 0;
        CGFloat feedHeight = MAX(measuredFeedHeight, dateHeight);
        CGFloat measuredPreviewHeight = fluxbar_measured_text_height(
            previewText,
            previewFont,
            contentWidth
        ) + 8;
        CGFloat previewHeight = MAX(26, measuredPreviewHeight);
        NSUInteger arrangedViews = 2 + (hasImage ? 1 : 0) + (hasMetadata ? 1 : 0);
        CGFloat fixedHeight = 2 * FluxBarPreviewVerticalPadding + imageHeight + titleHeight + feedHeight +
            FluxBarPreviewSpacing * (arrangedViews - 1);
        CGFloat naturalHeight = fixedHeight + previewHeight;

        selected.width = width;
        selected.height = MIN(naturalHeight, maximumHeight);
        selected.contentWidth = contentWidth;
        selected.imageHeight = imageHeight;
        selected.titleHeight = titleHeight;
        selected.feedHeight = feedHeight;
        selected.feedWidth = feedWidth;
        selected.dateWidth = dateWidth;
        selected.previewHeight = MAX(26, selected.height - fixedHeight);
        selected.previewScrolls = measuredPreviewHeight > selected.previewHeight + 1;

        if (naturalHeight <= maximumHeight || width + 20 > maximumWidth) {
            break;
        }
    }
    return selected;
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
    NSRect visibleFrame = fluxbar_visible_frame_for_point(NSEvent.mouseLocation);
    FluxBarPreviewLayout layout = fluxbar_preview_layout(article, hasImage, visibleFrame);

    NSVisualEffectView *root = [[NSVisualEffectView alloc]
        initWithFrame:NSMakeRect(0, 0, layout.width, layout.height)];
    root.material = NSVisualEffectMaterialPopover;
    root.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    root.state = NSVisualEffectStateActive;
    root.wantsLayer = YES;
    root.layer.cornerRadius = 12;
    root.layer.masksToBounds = YES;

    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = FluxBarPreviewSpacing;
    stack.edgeInsets = NSEdgeInsetsMake(
        FluxBarPreviewVerticalPadding,
        FluxBarPreviewHorizontalPadding,
        FluxBarPreviewVerticalPadding,
        FluxBarPreviewHorizontalPadding
    );
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
        [imageView.heightAnchor constraintEqualToConstant:layout.imageHeight].active = YES;
        [imageView.widthAnchor constraintEqualToConstant:layout.contentWidth].active = YES;
        [stack addArrangedSubview:imageView];
    }

    NSTextField *titleLabel = fluxbar_label(
        article.title,
        [NSFont boldSystemFontOfSize:15],
        NSColor.labelColor
    );
    titleLabel.maximumNumberOfLines = 3;
    titleLabel.lineBreakMode = NSLineBreakByTruncatingTail;
    [titleLabel.heightAnchor constraintEqualToConstant:layout.titleHeight].active = YES;
    [titleLabel.widthAnchor constraintEqualToConstant:layout.contentWidth].active = YES;
    [stack addArrangedSubview:titleLabel];
    NSString *publishedDate = fluxbar_published_date(article.publishedAt);
    if (article.feed.length > 0 || publishedDate.length > 0) {
        NSView *metadataView = [[NSView alloc] init];
        metadataView.translatesAutoresizingMaskIntoConstraints = NO;
        [metadataView.heightAnchor constraintEqualToConstant:layout.feedHeight].active = YES;
        [metadataView.widthAnchor constraintEqualToConstant:layout.contentWidth].active = YES;

        if (article.feed.length > 0) {
            NSTextField *feedLabel = fluxbar_label(
                article.feed,
                [NSFont systemFontOfSize:12],
                NSColor.secondaryLabelColor
            );
            feedLabel.maximumNumberOfLines = 2;
            feedLabel.lineBreakMode = NSLineBreakByTruncatingTail;
            feedLabel.translatesAutoresizingMaskIntoConstraints = NO;
            [metadataView addSubview:feedLabel];
            [NSLayoutConstraint activateConstraints:@[
                [feedLabel.leadingAnchor constraintEqualToAnchor:metadataView.leadingAnchor],
                [feedLabel.topAnchor constraintEqualToAnchor:metadataView.topAnchor],
                [feedLabel.widthAnchor constraintEqualToConstant:layout.feedWidth]
            ]];
        }
        if (publishedDate.length > 0) {
            NSTextField *dateLabel = [NSTextField labelWithString:publishedDate];
            dateLabel.font = [NSFont systemFontOfSize:12];
            dateLabel.textColor = NSColor.secondaryLabelColor;
            dateLabel.alignment = NSTextAlignmentRight;
            dateLabel.lineBreakMode = NSLineBreakByTruncatingTail;
            dateLabel.translatesAutoresizingMaskIntoConstraints = NO;
            [metadataView addSubview:dateLabel];
            [NSLayoutConstraint activateConstraints:@[
                [dateLabel.trailingAnchor constraintEqualToAnchor:metadataView.trailingAnchor],
                [dateLabel.topAnchor constraintEqualToAnchor:metadataView.topAnchor],
                [dateLabel.widthAnchor constraintEqualToConstant:layout.dateWidth]
            ]];
        }
        [stack addArrangedSubview:metadataView];
    }

    NSScrollView *scrollView = [[NSScrollView alloc] init];
    scrollView.hasVerticalScroller = layout.previewScrolls;
    scrollView.autohidesScrollers = YES;
    scrollView.drawsBackground = NO;
    scrollView.borderType = NSNoBorder;
    scrollView.translatesAutoresizingMaskIntoConstraints = NO;
    [scrollView.heightAnchor constraintEqualToConstant:layout.previewHeight].active = YES;
    [scrollView.widthAnchor constraintEqualToConstant:layout.contentWidth].active = YES;

    NSTextView *textView = [[NSTextView alloc]
        initWithFrame:NSMakeRect(0, 0, layout.contentWidth, layout.previewHeight)];
    textView.string = article.preview.length > 0
        ? article.preview
        : FluxBarLocalized(@"preview.no_text", @"No text preview available.");
    textView.font = [NSFont systemFontOfSize:13];
    textView.textColor = NSColor.labelColor;
    textView.editable = NO;
    textView.selectable = NO;
    textView.drawsBackground = NO;
    textView.textContainerInset = NSMakeSize(0, 4);
    textView.verticallyResizable = YES;
    textView.horizontallyResizable = NO;
    textView.minSize = NSMakeSize(0, layout.previewHeight);
    textView.maxSize = NSMakeSize(CGFLOAT_MAX, CGFLOAT_MAX);
    textView.autoresizingMask = NSViewWidthSizable;
    textView.textContainer.containerSize = NSMakeSize(layout.contentWidth, CGFLOAT_MAX);
    textView.textContainer.widthTracksTextView = YES;
    scrollView.documentView = textView;
    [stack addArrangedSubview:scrollView];

    NSPanel *panel = [[NSPanel alloc]
        initWithContentRect:NSMakeRect(0, 0, layout.width, layout.height)
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
    long long publishedUnix,
    const char *previewValue,
    const char *imageURLValue,
    const unsigned char *fallbackIconBytes,
    int fallbackIconLength
) {
    FluxBarArticlePreview *article = [[FluxBarArticlePreview alloc] init];
    article.title = fluxbar_string(titleValue);
    article.feed = fluxbar_string(feedValue);
    article.publishedAt = publishedUnix > 0
        ? [NSDate dateWithTimeIntervalSince1970:(NSTimeInterval)publishedUnix]
        : nil;
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
