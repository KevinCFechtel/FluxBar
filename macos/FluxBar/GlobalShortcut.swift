import Carbon
import Foundation

private let fluxBarHotKeyID = EventHotKeyID(signature: 0x464C5558, id: 1) // FLUX

enum GlobalShortcutChoice: String, CaseIterable, Sendable {
    case optionCommandF
    case controlOptionF
    case disabled

    private static let defaultsKey = "FluxBar.globalShortcut"

    static func stored(in defaults: UserDefaults) -> GlobalShortcutChoice {
        defaults.string(forKey: defaultsKey).flatMap(GlobalShortcutChoice.init(rawValue:)) ?? .optionCommandF
    }

    func store(in defaults: UserDefaults) {
        defaults.set(rawValue, forKey: Self.defaultsKey)
    }

    @MainActor
    func title(localization: Localization) -> String {
        switch self {
        case .optionCommandF:
            return localization.text("settings.shortcut.option_command_f", "Option-Command-F")
        case .controlOptionF:
            return localization.text("settings.shortcut.control_option_f", "Control-Option-F")
        case .disabled:
            return localization.text("settings.shortcut.disabled", "Disabled")
        }
    }

    fileprivate var registration: (keyCode: UInt32, modifiers: UInt32)? {
        switch self {
        case .optionCommandF:
            return (UInt32(kVK_ANSI_F), UInt32(optionKey | cmdKey))
        case .controlOptionF:
            return (UInt32(kVK_ANSI_F), UInt32(controlKey | optionKey))
        case .disabled:
            return nil
        }
    }
}

@MainActor
final class GlobalShortcutRegistrar {
    private var eventHandler: EventHandlerRef?
    private var hotKey: EventHotKeyRef?
    private var eventHandlerStatus = OSStatus(eventNotHandledErr)
    private let action: () -> Void

    init(action: @escaping () -> Void) {
        self.action = action
        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        eventHandlerStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            { _, event, userData in
                guard let event, let userData else { return OSStatus(eventNotHandledErr) }
                var identifier = EventHotKeyID()
                let status = GetEventParameter(
                    event,
                    EventParamName(kEventParamDirectObject),
                    EventParamType(typeEventHotKeyID),
                    nil,
                    MemoryLayout<EventHotKeyID>.size,
                    nil,
                    &identifier
                )
                guard status == noErr,
                      identifier.signature == fluxBarHotKeyID.signature,
                      identifier.id == fluxBarHotKeyID.id else {
                    return OSStatus(eventNotHandledErr)
                }
                let registrar = Unmanaged<GlobalShortcutRegistrar>.fromOpaque(userData).takeUnretainedValue()
                DispatchQueue.main.async { registrar.action() }
                return noErr
            },
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &eventHandler
        )
    }

    deinit {
        if let hotKey { UnregisterEventHotKey(hotKey) }
        if let eventHandler { RemoveEventHandler(eventHandler) }
    }

    func register(_ shortcut: GlobalShortcutChoice) -> OSStatus {
        guard eventHandlerStatus == noErr else { return eventHandlerStatus }
        guard let registration = shortcut.registration else {
            if let hotKey { UnregisterEventHotKey(hotKey) }
            self.hotKey = nil
            return noErr
        }
        var replacement: EventHotKeyRef?
        let status = RegisterEventHotKey(
            registration.keyCode,
            registration.modifiers,
            fluxBarHotKeyID,
            GetApplicationEventTarget(),
            UInt32(kEventHotKeyExclusive),
            &replacement
        )
        guard status == noErr else { return status }
        if let hotKey { UnregisterEventHotKey(hotKey) }
        hotKey = replacement
        return noErr
    }

    func sendTestEventForSmokeTest() -> OSStatus {
        var event: EventRef?
        let creationStatus = CreateEvent(
            nil,
            OSType(kEventClassKeyboard),
            UInt32(kEventHotKeyPressed),
            GetCurrentEventTime(),
            0,
            &event
        )
        guard creationStatus == noErr, let event else { return creationStatus }
        var identifier = fluxBarHotKeyID
        let parameterStatus = withUnsafePointer(to: &identifier) { pointer in
            SetEventParameter(
                event,
                EventParamName(kEventParamDirectObject),
                EventParamType(typeEventHotKeyID),
                MemoryLayout<EventHotKeyID>.size,
                pointer
            )
        }
        guard parameterStatus == noErr else { return parameterStatus }
        return SendEventToEventTarget(event, GetApplicationEventTarget())
    }
}
