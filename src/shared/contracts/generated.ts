// Сгенерировано из io-core. Обновить: npm run contracts

export type DeviceAccess = "notImplemented" | "available";

export type AppInfo = { name: string, version: string, platform: string, deviceAccess: DeviceAccess, };
export type AppError = { code: string, message: string, };

export type DeviceIdentity = { name: string, vendorId: number, productId: number, firmware: string, frameVersion: number, rtPrecision: number, };

export type Rgb = { r: number, g: number, b: number, };

export type BindingRecord = { page: number, parameters: [number, number, number], };

export type Actuation = { triggerUm: number, pressUm: number, releaseUm: number, rapidTrigger: boolean, wholeTravel: boolean, rampage: boolean, axisType: number, };

export type KeyConfiguration = { slot: number, base: BindingRecord, function: BindingRecord, actuation: Actuation, ledId: number, color: Rgb, };

export type LightingSettings = { mode: number, color: Rgb, secondaryColor: Rgb, colorMode: number, brightness: number, speed: number, direction: number, };

export type PerformanceSettings = { reportRate: number, topDeadZoneUm: number, bottomDeadZoneUm: number, keyDelay: number, };

export type MacroStep = { keyCode: number, pressed: boolean, delayMs: number, kind: number, };

export type HardwareMacro = { id: number, steps: Array<MacroStep>, };

export type DksConfiguration = { index: number, thresholds: [number, number, number, number], actions: [number, number, number, number], states: [number, number, number, number], };

export type KeyboardSnapshot = { revision: string, identity: DeviceIdentity, keys: Array<KeyConfiguration>, lighting: LightingSettings, performance: PerformanceSettings, macros: Array<HardwareMacro>, dks: Array<DksConfiguration>, macroBytesUsed: number, macroWriteLimit: number, };

export type Edit = { "kind": "binding", slots: Array<number>, functionLayer: boolean, binding: BindingRecord, } | { "kind": "actuation", slots: Array<number>, value: Actuation, } | { "kind": "lighting", value: LightingSettings, } | { "kind": "color", slots: Array<number>, color: Rgb, } | { "kind": "performance", value: PerformanceSettings, } | { "kind": "macros", values: Array<HardwareMacro>, } | { "kind": "dks", value: DksConfiguration, };

export type ChangeRequest = { baseRevision: string, edits: Array<Edit>, };

export type ChangeSummary = { block: string, changedBytes: number, };

export type ChangePreview = { token: string, changes: Array<ChangeSummary>, };

export type ApplyResult = { snapshot: KeyboardSnapshot, recoveryAvailable: boolean, };

export type KeyTravel = { slot: number, travelUm: number, maxTravelUm: number, adc: number, ageMs: number, };

export type KeyPress = { sequence: number, slot: number, peakUm: number, };

export type LiveColor = { ledId: number, color: Rgb, };

export type MonitorFrame = { active: boolean, travel: Array<KeyTravel>, history: Array<KeyPress>, colors: Array<LiveColor>, colorAgeMs: number | null, packets: number, message: string | null, };
