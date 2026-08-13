//go:build darwin

#import <Cocoa/Cocoa.h>
#import <Security/Security.h>
#import <dispatch/dispatch.h>
#import <stdbool.h>
#import <stdlib.h>
#import <string.h>

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
    return CFBridgingRelease(message) ?: [NSString stringWithFormat:@"Keychain-Fehler %d", (int)status];
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

int fluxbar_load_settings(char **server, char **apiKey, bool *showSplash, char **errorMessage) {
    *server = NULL;
    *apiKey = NULL;
    *showSplash = true;
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
    if (storedServer == nil || storedAPIKey == nil) {
        *errorMessage = fluxbar_copy_utf8(@"Die gespeicherten Zugangsdaten sind beschädigt.");
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
    return 1;
}

bool fluxbar_save_settings(const char *serverValue, const char *apiKeyValue, bool showSplash, char **errorMessage) {
    *errorMessage = NULL;
    NSString *server = [NSString stringWithUTF8String:serverValue ?: ""] ?: @"";
    NSString *apiKey = [NSString stringWithUTF8String:apiKeyValue ?: ""] ?: @"";
    NSData *jsonData = [NSJSONSerialization dataWithJSONObject:@{
        @"server": server,
        @"apiKey": apiKey,
        @"showSplash": @(showSplash),
    }
                                                       options:0
                                                         error:nil];
    NSString *json = [[NSString alloc] initWithData:jsonData encoding:NSUTF8StringEncoding];
    if (json == nil) {
        *errorMessage = fluxbar_copy_utf8(@"Die Zugangsdaten konnten nicht serialisiert werden.");
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

    NSMenu *editMenu = [[NSMenu alloc] initWithTitle:@"Bearbeiten"];
    editMenu.identifier = @"FluxBarEditMenu";
    [editMenu addItem:fluxbar_edit_menu_item(@"Widerrufen", @selector(undo:), @"z")];
    [editMenu addItem:fluxbar_edit_menu_item(@"Wiederholen", @selector(redo:), @"Z")];
    [editMenu addItem:[NSMenuItem separatorItem]];
    [editMenu addItem:fluxbar_edit_menu_item(@"Ausschneiden", @selector(cut:), @"x")];
    [editMenu addItem:fluxbar_edit_menu_item(@"Kopieren", @selector(copy:), @"c")];
    [editMenu addItem:fluxbar_edit_menu_item(@"Einsetzen", @selector(paste:), @"v")];
    [editMenu addItem:fluxbar_edit_menu_item(@"Alles auswählen", @selector(selectAll:), @"a")];

    NSMenuItem *editRoot = [[NSMenuItem alloc] initWithTitle:@"Bearbeiten" action:nil keyEquivalent:@""];
    editRoot.submenu = editMenu;
    [mainMenu addItem:editRoot];
}

int fluxbar_prompt_settings(
    const char *serverValue,
    const char *apiKeyValue,
    bool showSplashValue,
    const char *validationValue,
    char **savedServer,
    char **savedAPIKey,
    bool *savedShowSplash
) {
    *savedServer = NULL;
    *savedAPIKey = NULL;
    *savedShowSplash = showSplashValue;
    __block int result = -1;
    fluxbar_settings_main_sync(^{
        NSString *server = [NSString stringWithUTF8String:serverValue ?: ""] ?: @"";
        NSString *apiKey = [NSString stringWithUTF8String:apiKeyValue ?: ""] ?: @"";
        NSString *validation = [NSString stringWithUTF8String:validationValue ?: ""] ?: @"";

        NSAlert *alert = [[NSAlert alloc] init];
        alert.messageText = @"FluxBar-Einstellungen";
        alert.informativeText = @"Die Zugangsdaten werden verschlüsselt im macOS-Schlüsselbund gespeichert.";
        [alert addButtonWithTitle:@"Speichern"];
        [alert addButtonWithTitle:@"Abbrechen"];

        CGFloat validationHeight = validation.length > 0 ? 34 : 0;
        NSView *form = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 430, 140 + validationHeight)];
        NSTextField *serverLabel = fluxbar_settings_label(@"Miniflux-Server", NSMakeRect(0, 110 + validationHeight, 125, 22));
        NSTextField *serverField = [[NSTextField alloc] initWithFrame:NSMakeRect(130, 108 + validationHeight, 300, 24)];
        serverField.stringValue = server;
        serverField.placeholderString = @"https://miniflux.example.com";
        NSTextField *keyLabel = fluxbar_settings_label(@"API-Key", NSMakeRect(0, 74 + validationHeight, 125, 22));
        NSSecureTextField *keyField = [[NSSecureTextField alloc] initWithFrame:NSMakeRect(130, 72 + validationHeight, 300, 24)];
        keyField.stringValue = apiKey;
        keyField.placeholderString = @"Miniflux API-Key";
        NSButton *showSplashButton = [NSButton checkboxWithTitle:@"Startanzeige beim Öffnen anzeigen"
                                                          target:nil
                                                          action:nil];
        showSplashButton.frame = NSMakeRect(130, 35 + validationHeight, 300, 24);
        showSplashButton.state = showSplashValue ? NSControlStateValueOn : NSControlStateValueOff;
        [form addSubview:serverLabel];
        [form addSubview:serverField];
        [form addSubview:keyLabel];
        [form addSubview:keyField];
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
            result = 1;
        } else {
            result = 0;
        }
    });
    return result;
}
