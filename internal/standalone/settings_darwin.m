//go:build darwin

#import <Cocoa/Cocoa.h>
#import <Security/Security.h>
#import <ServiceManagement/ServiceManagement.h>
#import <dispatch/dispatch.h>
#import <stdbool.h>
#import <stdlib.h>
#import <string.h>
#import "localization_darwin.h"

static NSString *const FluxBarKeychainService = @"dev.kevincfechtel.FluxBar.miniflux";
static NSString *const FluxBarLegacyKeychainService = @"com.kevinfechtel.FluxBar.miniflux";
static NSString *const FluxBarCredentialsAccount = @"credentials";

static void fluxbar_settings_main_sync(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_sync(dispatch_get_main_queue(), block);
    }
}

static char *fluxbar_copy_utf8(NSString *value) {
    const char *utf8 = (value ?: @"").UTF8String;
    return strdup(utf8 != NULL ? utf8 : "");
}

static NSString *fluxbar_keychain_error(OSStatus status) {
    CFStringRef message = SecCopyErrorMessageString(status, NULL);
    return CFBridgingRelease(message) ?: [NSString
        stringWithFormat:FluxBarLocalized(@"error.keychain_format", @"Keychain error %d"),
                         (int)status];
}

static OSStatus fluxbar_read_keychain_value(NSString *service, NSString *account, NSString **value) {
    NSDictionary *query = @{
        (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService: service,
        (__bridge id)kSecAttrAccount: account,
        (__bridge id)kSecReturnData: @YES,
        (__bridge id)kSecMatchLimit: (__bridge id)kSecMatchLimitOne,
    };
    CFTypeRef result = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    if (status != errSecSuccess) {
        return status;
    }
    NSData *data = CFBridgingRelease(result);
    NSString *decoded = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    if (decoded == nil) {
        return errSecDecode;
    }
    *value = decoded;
    return errSecSuccess;
}

static OSStatus fluxbar_write_keychain_value(NSString *service, NSString *account, NSString *value) {
    NSData *data = [value dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *query = @{
        (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService: service,
        (__bridge id)kSecAttrAccount: account,
    };
    NSDictionary *update = @{(__bridge id)kSecValueData: data};
    OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)update);
    if (status == errSecItemNotFound) {
        NSMutableDictionary *item = [query mutableCopy];
        item[(__bridge id)kSecValueData] = data;
        item[(__bridge id)kSecAttrAccessible] = (__bridge id)kSecAttrAccessibleWhenUnlocked;
        status = SecItemAdd((__bridge CFDictionaryRef)item, NULL);
    }
    return status;
}

static bool fluxbar_login_item_supported(void) {
    if (@available(macOS 13.0, *)) {
        return true;
    }
    return false;
}

static bool fluxbar_launch_at_login_enabled(void) {
    if (@available(macOS 13.0, *)) {
        return SMAppService.mainAppService.status == SMAppServiceStatusEnabled;
    }
    return false;
}

static bool fluxbar_update_launch_at_login(bool enabled, NSString **errorMessage) {
    if (@available(macOS 13.0, *)) {
        SMAppService *service = SMAppService.mainAppService;
        SMAppServiceStatus status = service.status;
        if (enabled && status == SMAppServiceStatusEnabled) {
            return true;
        }
        if (!enabled && (status == SMAppServiceStatusNotRegistered || status == SMAppServiceStatusNotFound)) {
            return true;
        }
        if (enabled && status == SMAppServiceStatusRequiresApproval) {
            [SMAppService openSystemSettingsLoginItems];
            *errorMessage = FluxBarLocalized(
                @"error.login_item_disabled",
                @"FluxBar was disabled in macOS Login Items. Please allow it there again."
            );
            return false;
        }

        NSError *error = nil;
        bool succeeded = enabled
            ? [service registerAndReturnError:&error]
            : [service unregisterAndReturnError:&error];
        if (!succeeded) {
            *errorMessage = error.localizedDescription ?: FluxBarLocalized(
                @"error.login_item_change_failed",
                @"The login item could not be changed."
            );
            return false;
        }
        if (enabled && service.status == SMAppServiceStatusRequiresApproval) {
            [SMAppService openSystemSettingsLoginItems];
            *errorMessage = FluxBarLocalized(
                @"error.login_item_approval",
                @"Please allow FluxBar in macOS Login Items."
            );
            return false;
        }
        return true;
    }
    if (enabled) {
        *errorMessage = FluxBarLocalized(
            @"error.login_item_unsupported",
            @"Launch at login requires macOS 13 or later."
        );
        return false;
    }
    return true;
}

int fluxbar_load_settings(
    char **server,
    char **apiKey,
    bool *showSplash,
    bool *launchAtLogin,
    bool *newestFirst,
    char **errorMessage
) {
    *server = NULL;
    *apiKey = NULL;
    *showSplash = true;
    *launchAtLogin = fluxbar_launch_at_login_enabled();
    *newestFirst = false;
    *errorMessage = NULL;

    NSString *storedJSON = nil;
    OSStatus status = fluxbar_read_keychain_value(
        FluxBarKeychainService,
        FluxBarCredentialsAccount,
        &storedJSON
    );
    bool loadedLegacySettings = false;
    if (status == errSecItemNotFound) {
        status = fluxbar_read_keychain_value(
            FluxBarLegacyKeychainService,
            FluxBarCredentialsAccount,
            &storedJSON
        );
        loadedLegacySettings = status == errSecSuccess;
        if (status == errSecItemNotFound) {
            return 0;
        }
    }
    if (status != errSecSuccess) {
        *errorMessage = fluxbar_copy_utf8(fluxbar_keychain_error(status));
        return -1;
    }
    NSData *jsonData = [storedJSON dataUsingEncoding:NSUTF8StringEncoding];
    id decoded = [NSJSONSerialization JSONObjectWithData:jsonData options:0 error:nil];
    NSDictionary *credentials = [decoded isKindOfClass:[NSDictionary class]] ? decoded : nil;
    NSString *storedServer = [credentials[@"server"] isKindOfClass:[NSString class]] ? credentials[@"server"] : nil;
    NSString *storedAPIKey = [credentials[@"apiKey"] isKindOfClass:[NSString class]] ? credentials[@"apiKey"] : nil;
    NSNumber *storedShowSplash = [credentials[@"showSplash"] isKindOfClass:[NSNumber class]] ? credentials[@"showSplash"] : nil;
    NSNumber *storedNewestFirst = [credentials[@"newestFirst"] isKindOfClass:[NSNumber class]] ? credentials[@"newestFirst"] : nil;
    if (storedServer == nil || storedAPIKey == nil) {
        *errorMessage = fluxbar_copy_utf8(FluxBarLocalized(
            @"error.credentials_corrupt",
            @"The stored credentials are corrupted."
        ));
        return -1;
    }
    if (loadedLegacySettings) {
        // Keep the legacy item as a recoverable fallback. A successful write
        // transparently moves future reads to the new application namespace.
        fluxbar_write_keychain_value(FluxBarKeychainService, FluxBarCredentialsAccount, storedJSON);
    }
    *server = fluxbar_copy_utf8(storedServer);
    *apiKey = fluxbar_copy_utf8(storedAPIKey);
    *showSplash = storedShowSplash == nil ? true : storedShowSplash.boolValue;
    *newestFirst = storedNewestFirst == nil ? false : storedNewestFirst.boolValue;
    return 1;
}

bool fluxbar_save_settings(
    const char *serverValue,
    const char *apiKeyValue,
    bool showSplash,
    bool launchAtLogin,
    bool newestFirst,
    char **errorMessage
) {
    *errorMessage = NULL;
    __block NSString *loginError = nil;
    __block bool loginUpdated = false;
    fluxbar_settings_main_sync(^{
        loginUpdated = fluxbar_update_launch_at_login(launchAtLogin, &loginError);
    });
    if (!loginUpdated) {
        *errorMessage = fluxbar_copy_utf8(loginError);
        return false;
    }

    NSString *server = [NSString stringWithUTF8String:serverValue ?: ""] ?: @"";
    NSString *apiKey = [NSString stringWithUTF8String:apiKeyValue ?: ""] ?: @"";
    NSData *jsonData = [NSJSONSerialization dataWithJSONObject:@{
        @"server": server,
        @"apiKey": apiKey,
        @"showSplash": @(showSplash),
        @"newestFirst": @(newestFirst),
    }
                                                       options:0
                                                         error:nil];
    NSString *json = [[NSString alloc] initWithData:jsonData encoding:NSUTF8StringEncoding];
    if (json == nil) {
        *errorMessage = fluxbar_copy_utf8(FluxBarLocalized(
            @"error.credentials_serialize",
            @"The credentials could not be serialized."
        ));
        return false;
    }
    OSStatus status = fluxbar_write_keychain_value(FluxBarKeychainService, FluxBarCredentialsAccount, json);
    if (status != errSecSuccess) {
        *errorMessage = fluxbar_copy_utf8(fluxbar_keychain_error(status));
        return false;
    }
    return true;
}

static NSTextField *fluxbar_settings_label(NSString *text, NSRect frame) {
    NSTextField *label = [[NSTextField alloc] initWithFrame:frame];
    label.stringValue = text;
    label.editable = NO;
    label.selectable = NO;
    label.bezeled = NO;
    label.drawsBackground = NO;
    label.font = [NSFont systemFontOfSize:13];
    return label;
}

static NSMenuItem *fluxbar_edit_menu_item(NSString *title, SEL action, NSString *keyEquivalent) {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title action:action keyEquivalent:keyEquivalent];
    item.target = nil;
    return item;
}

static void fluxbar_install_edit_menu(void) {
    NSMenu *mainMenu = NSApp.mainMenu;
    if (mainMenu == nil) {
        mainMenu = [[NSMenu alloc] initWithTitle:@""];
        NSApp.mainMenu = mainMenu;
    }
    for (NSMenuItem *item in mainMenu.itemArray) {
        if (item.submenu != nil && [item.submenu.identifier isEqualToString:@"FluxBarEditMenu"]) {
            return;
        }
    }

    NSString *editTitle = FluxBarLocalized(@"edit.menu", @"Edit");
    NSMenu *editMenu = [[NSMenu alloc] initWithTitle:editTitle];
    editMenu.identifier = @"FluxBarEditMenu";
    [editMenu addItem:fluxbar_edit_menu_item(FluxBarLocalized(@"edit.undo", @"Undo"), @selector(undo:), @"z")];
    [editMenu addItem:fluxbar_edit_menu_item(FluxBarLocalized(@"edit.redo", @"Redo"), @selector(redo:), @"Z")];
    [editMenu addItem:[NSMenuItem separatorItem]];
    [editMenu addItem:fluxbar_edit_menu_item(FluxBarLocalized(@"edit.cut", @"Cut"), @selector(cut:), @"x")];
    [editMenu addItem:fluxbar_edit_menu_item(FluxBarLocalized(@"edit.copy", @"Copy"), @selector(copy:), @"c")];
    [editMenu addItem:fluxbar_edit_menu_item(FluxBarLocalized(@"edit.paste", @"Paste"), @selector(paste:), @"v")];
    [editMenu addItem:fluxbar_edit_menu_item(FluxBarLocalized(@"edit.select_all", @"Select All"), @selector(selectAll:), @"a")];

    NSMenuItem *editRoot = [[NSMenuItem alloc] initWithTitle:editTitle action:nil keyEquivalent:@""];
    editRoot.submenu = editMenu;
    [mainMenu addItem:editRoot];
}

int fluxbar_prompt_settings(
    const char *serverValue,
    const char *apiKeyValue,
    bool showSplashValue,
    bool launchAtLoginValue,
    bool newestFirstValue,
    const char *validationValue,
    char **savedServer,
    char **savedAPIKey,
    bool *savedShowSplash,
    bool *savedLaunchAtLogin,
    bool *savedNewestFirst
) {
    *savedServer = NULL;
    *savedAPIKey = NULL;
    *savedShowSplash = showSplashValue;
    *savedLaunchAtLogin = launchAtLoginValue;
    *savedNewestFirst = newestFirstValue;
    __block int result = -1;
    fluxbar_settings_main_sync(^{
        bool currentLaunchAtLogin = fluxbar_launch_at_login_enabled();
        NSString *server = [NSString stringWithUTF8String:serverValue ?: ""] ?: @"";
        NSString *apiKey = [NSString stringWithUTF8String:apiKeyValue ?: ""] ?: @"";
        NSString *validation = [NSString stringWithUTF8String:validationValue ?: ""] ?: @"";

        NSAlert *alert = [[NSAlert alloc] init];
        alert.messageText = FluxBarLocalized(@"settings.title", @"FluxBar Settings");
        alert.informativeText = FluxBarLocalized(
            @"settings.security_note",
            @"Credentials are stored securely in the macOS Keychain."
        );
        [alert addButtonWithTitle:FluxBarLocalized(@"settings.save", @"Save")];
        [alert addButtonWithTitle:FluxBarLocalized(@"settings.cancel", @"Cancel")];

        CGFloat validationHeight = validation.length > 0 ? 34 : 0;
        NSView *form = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 430, 212 + validationHeight)];
        NSTextField *serverLabel = fluxbar_settings_label(
            FluxBarLocalized(@"settings.server", @"Miniflux Server"),
            NSMakeRect(0, 182 + validationHeight, 125, 22)
        );
        NSTextField *serverField = [[NSTextField alloc] initWithFrame:NSMakeRect(130, 180 + validationHeight, 300, 24)];
        serverField.stringValue = server;
        serverField.placeholderString = @"https://miniflux.example.com";
        NSTextField *keyLabel = fluxbar_settings_label(
            FluxBarLocalized(@"settings.api_key", @"API Key"),
            NSMakeRect(0, 146 + validationHeight, 125, 22)
        );
        NSSecureTextField *keyField = [[NSSecureTextField alloc] initWithFrame:NSMakeRect(130, 144 + validationHeight, 300, 24)];
        keyField.stringValue = apiKey;
        keyField.placeholderString = FluxBarLocalized(
            @"settings.api_key_placeholder",
            @"Miniflux API Key"
        );
        NSTextField *sortLabel = fluxbar_settings_label(
            FluxBarLocalized(@"settings.sort", @"Sort Order"),
            NSMakeRect(0, 110 + validationHeight, 125, 22)
        );
        NSPopUpButton *sortButton = [[NSPopUpButton alloc]
            initWithFrame:NSMakeRect(130, 106 + validationHeight, 220, 28)
                pullsDown:NO];
        [sortButton addItemsWithTitles:@[
            FluxBarLocalized(@"settings.sort.newest", @"Newest First"),
            FluxBarLocalized(@"settings.sort.oldest", @"Oldest First")
        ]];
        [sortButton selectItemAtIndex:newestFirstValue ? 0 : 1];
        NSButton *launchAtLoginButton = [NSButton checkboxWithTitle:FluxBarLocalized(
                                                                 @"settings.launch_at_login",
                                                                 @"Launch automatically at login"
                                                             )
                                                             target:nil
                                                             action:nil];
        launchAtLoginButton.frame = NSMakeRect(130, 70 + validationHeight, 300, 24);
        launchAtLoginButton.state = currentLaunchAtLogin ? NSControlStateValueOn : NSControlStateValueOff;
        launchAtLoginButton.enabled = fluxbar_login_item_supported();
        if (!launchAtLoginButton.enabled) {
            launchAtLoginButton.toolTip = FluxBarLocalized(
                @"settings.launch_at_login.unsupported",
                @"This option requires macOS 13 or later."
            );
        }
        NSButton *showSplashButton = [NSButton checkboxWithTitle:FluxBarLocalized(
                                                            @"settings.show_startup",
                                                            @"Show startup notification when opening"
                                                        )
                                                          target:nil
                                                          action:nil];
        showSplashButton.frame = NSMakeRect(130, 35 + validationHeight, 300, 24);
        showSplashButton.state = showSplashValue ? NSControlStateValueOn : NSControlStateValueOff;
        [form addSubview:serverLabel];
        [form addSubview:serverField];
        [form addSubview:keyLabel];
        [form addSubview:keyField];
        [form addSubview:sortLabel];
        [form addSubview:sortButton];
        [form addSubview:launchAtLoginButton];
        [form addSubview:showSplashButton];
        if (validation.length > 0) {
            NSTextField *validationLabel = fluxbar_settings_label(validation, NSMakeRect(0, 0, 430, 30));
            validationLabel.textColor = NSColor.systemRedColor;
            validationLabel.maximumNumberOfLines = 2;
            validationLabel.lineBreakMode = NSLineBreakByWordWrapping;
            [form addSubview:validationLabel];
        }
        alert.accessoryView = form;
        alert.window.initialFirstResponder = serverField;

        fluxbar_install_edit_menu();
        [NSApp activateIgnoringOtherApps:YES];
        NSModalResponse response = [alert runModal];
        if (response == NSAlertFirstButtonReturn) {
            *savedServer = fluxbar_copy_utf8(serverField.stringValue);
            *savedAPIKey = fluxbar_copy_utf8(keyField.stringValue);
            *savedShowSplash = showSplashButton.state == NSControlStateValueOn;
            *savedLaunchAtLogin = launchAtLoginButton.state == NSControlStateValueOn;
            *savedNewestFirst = sortButton.indexOfSelectedItem == 0;
            result = 1;
        } else {
            result = 0;
        }
    });
    return result;
}
