import Foundation
import SwiftData

enum AnimalType: String, Codable, CaseIterable, Identifiable {
    case cattle, dairyCow, beefCattle, chicken, duck, turkey, goose, quail
    case guineaFowl, goat, sheep, pig, horse, donkey, mule, rabbit
    case llama, alpaca, bison, waterBuffalo, deer, emu, ostrich, fish
    case beeColony, other

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .cattle: return "Cattle"
        case .dairyCow: return "Dairy cow"
        case .beefCattle: return "Beef cattle"
        case .chicken: return "Chicken"
        case .duck: return "Duck"
        case .turkey: return "Turkey"
        case .goose: return "Goose"
        case .quail: return "Quail"
        case .guineaFowl: return "Guinea fowl"
        case .goat: return "Goat"
        case .sheep: return "Sheep"
        case .pig: return "Pig"
        case .horse: return "Horse"
        case .donkey: return "Donkey"
        case .mule: return "Mule"
        case .rabbit: return "Rabbit"
        case .llama: return "Llama"
        case .alpaca: return "Alpaca"
        case .bison: return "Bison"
        case .waterBuffalo: return "Water buffalo"
        case .deer: return "Deer"
        case .emu: return "Emu"
        case .ostrich: return "Ostrich"
        case .fish: return "Fish"
        case .beeColony: return "Bee colony"
        case .other: return "Other"
        }
    }

    var gestationDays: Int {
        switch self {
        case .cattle, .dairyCow, .beefCattle: return 283
        case .sheep: return 147
        case .goat: return 150
        case .pig: return 114
        case .horse, .donkey, .mule: return 340
        case .rabbit: return 31
        case .llama, .alpaca: return 345
        case .bison: return 285
        case .waterBuffalo: return 310
        case .deer: return 230
        case .chicken: return 21
        case .duck, .turkey, .guineaFowl: return 28
        case .goose: return 30
        case .quail: return 17
        case .emu: return 50
        case .ostrich: return 42
        case .fish, .beeColony: return 0
        case .other: return 120
        }
    }
}

enum FeedFrequency: String, Codable, CaseIterable, Identifiable {
    case daily, twiceDaily, weekly, custom
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .twiceDaily: return "Twice daily"
        default: return rawValue.capitalized
        }
    }
}

enum BreedingMethod: String, Codable, CaseIterable, Identifiable {
    case natural, artificialInsemination, embryoTransfer
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .natural: return "Natural"
        case .artificialInsemination: return "Artificial insemination"
        case .embryoTransfer: return "Embryo transfer"
        }
    }
}

enum BreedingStatus: String, Codable, CaseIterable, Identifiable {
    case planned, bred, pregnant, dueSoon, completed, failed
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .dueSoon: return "Due soon"
        default: return rawValue.capitalized
        }
    }
}

enum ArrivalOrigin: String, Codable, CaseIterable, Identifiable {
    case purchased, bornOnFarm, transferredIn, other
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .purchased: return "Purchased"
        case .bornOnFarm: return "Born on farm"
        case .transferredIn: return "Transferred in"
        case .other: return "Other"
        }
    }
    var dateLabel: String {
        switch self {
        case .bornOnFarm: return "Birth"
        case .purchased: return "Acquire"
        case .transferredIn: return "Transfer"
        case .other: return "Arrival"
        }
    }
}

enum RegistrationStatus: String, Codable, CaseIterable, Identifiable {
    case notRequired, pending, registered, expired
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .notRequired: return "Not required"
        default: return rawValue.capitalized
        }
    }
}

enum TransactionType: String, Codable, CaseIterable, Identifiable {
    case income, expense
    var id: String { rawValue }
}

enum ExpenseCategory: String, Codable, CaseIterable, Identifiable {
    case feed, veterinary, labor, equipment, utilities, livestockPurchase, other
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .livestockPurchase: return "Livestock purchase"
        default: return rawValue.capitalized
        }
    }
}

enum IncomeCategory: String, Codable, CaseIterable, Identifiable {
    case livestockSale, eggs, milk, meat, produce, other
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .livestockSale: return "Livestock sale"
        default: return rawValue.capitalized
        }
    }
}

@Model
final class FarmProfile {
    var farmName: String
    init(farmName: String = "Gut Farms") {
        self.farmName = farmName
    }
}

@Model
final class AnimalGroup {
    var name: String
    var typeRaw: String
    var count: Int
    var notes: String
    var purchaseCost: Double
    var createdAt: Date

    var type: AnimalType {
        get { AnimalType(rawValue: typeRaw) ?? .other }
        set { typeRaw = newValue.rawValue }
    }

    init(
        name: String,
        type: AnimalType,
        count: Int,
        notes: String = "",
        purchaseCost: Double = 0,
        createdAt: Date = .now
    ) {
        self.name = name
        self.typeRaw = type.rawValue
        self.count = count
        self.notes = notes
        self.purchaseCost = purchaseCost
        self.createdAt = createdAt
    }
}

@Model
final class FeedingSchedule {
    var animalGroupId: PersistentIdentifier?
    var animalGroupName: String
    var feedName: String
    var amountKg: Double
    var costPerKg: Double
    var frequencyRaw: String
    var timeOfDay: String
    var notes: String
    var active: Bool

    var frequency: FeedFrequency {
        get { FeedFrequency(rawValue: frequencyRaw) ?? .daily }
        set { frequencyRaw = newValue.rawValue }
    }

    var dailyCost: Double {
        switch frequency {
        case .daily: return amountKg * costPerKg
        case .twiceDaily: return amountKg * costPerKg * 2
        case .weekly: return (amountKg * costPerKg) / 7.0
        case .custom: return amountKg * costPerKg
        }
    }

    var monthlyCost: Double { dailyCost * 30.0 }

    init(
        animalGroupId: PersistentIdentifier? = nil,
        animalGroupName: String,
        feedName: String,
        amountKg: Double,
        costPerKg: Double,
        frequency: FeedFrequency,
        timeOfDay: String,
        notes: String = "",
        active: Bool = true
    ) {
        self.animalGroupId = animalGroupId
        self.animalGroupName = animalGroupName
        self.feedName = feedName
        self.amountKg = amountKg
        self.costPerKg = costPerKg
        self.frequencyRaw = frequency.rawValue
        self.timeOfDay = timeOfDay
        self.notes = notes
        self.active = active
    }
}

@Model
final class BreedingSchedule {
    var animalGroupId: PersistentIdentifier?
    var animalGroupName: String
    var animalTypeRaw: String
    var femaleLabel: String
    var sireName: String
    var methodRaw: String
    var statusRaw: String
    var breedingDate: Date
    var expectedDueDate: Date
    var expectedOffspring: Int
    var notes: String
    var active: Bool

    var method: BreedingMethod {
        get { BreedingMethod(rawValue: methodRaw) ?? .natural }
        set { methodRaw = newValue.rawValue }
    }

    var status: BreedingStatus {
        get { BreedingStatus(rawValue: statusRaw) ?? .planned }
        set { statusRaw = newValue.rawValue }
    }

    var animalType: AnimalType {
        get { AnimalType(rawValue: animalTypeRaw) ?? .other }
        set { animalTypeRaw = newValue.rawValue }
    }

    var daysUntilDue: Int {
        Calendar.current.dateComponents([.day], from: Calendar.current.startOfDay(for: .now), to: Calendar.current.startOfDay(for: expectedDueDate)).day ?? 0
    }

    init(
        animalGroupId: PersistentIdentifier? = nil,
        animalGroupName: String,
        animalType: AnimalType,
        femaleLabel: String,
        sireName: String = "",
        method: BreedingMethod = .natural,
        status: BreedingStatus = .planned,
        breedingDate: Date,
        expectedDueDate: Date,
        expectedOffspring: Int = 1,
        notes: String = "",
        active: Bool = true
    ) {
        self.animalGroupId = animalGroupId
        self.animalGroupName = animalGroupName
        self.animalTypeRaw = animalType.rawValue
        self.femaleLabel = femaleLabel
        self.sireName = sireName
        self.methodRaw = method.rawValue
        self.statusRaw = status.rawValue
        self.breedingDate = breedingDate
        self.expectedDueDate = expectedDueDate
        self.expectedOffspring = expectedOffspring
        self.notes = notes
        self.active = active
    }
}

@Model
final class AnimalArrival {
    var name: String
    var typeRaw: String
    var originRaw: String
    var eventDate: Date
    var registrationStatusRaw: String
    var registrationId: String
    var groupName: String
    var notes: String
    var createdAt: Date

    var type: AnimalType {
        get { AnimalType(rawValue: typeRaw) ?? .other }
        set { typeRaw = newValue.rawValue }
    }

    var origin: ArrivalOrigin {
        get { ArrivalOrigin(rawValue: originRaw) ?? .purchased }
        set { originRaw = newValue.rawValue }
    }

    var registrationStatus: RegistrationStatus {
        get { RegistrationStatus(rawValue: registrationStatusRaw) ?? .pending }
        set { registrationStatusRaw = newValue.rawValue }
    }

    var displayName: String {
        name.isEmpty ? "Unnamed \(type.displayName.lowercased())" : name
    }

    init(
        name: String = "",
        type: AnimalType,
        origin: ArrivalOrigin,
        eventDate: Date,
        registrationStatus: RegistrationStatus = .pending,
        registrationId: String = "",
        groupName: String = "",
        notes: String = "",
        createdAt: Date = .now
    ) {
        self.name = name
        self.typeRaw = type.rawValue
        self.originRaw = origin.rawValue
        self.eventDate = eventDate
        self.registrationStatusRaw = registrationStatus.rawValue
        self.registrationId = registrationId
        self.groupName = groupName
        self.notes = notes
        self.createdAt = createdAt
    }
}

@Model
final class FarmTransaction {
    var typeRaw: String
    var amount: Double
    var detail: String
    var expenseCategoryRaw: String?
    var incomeCategoryRaw: String?
    var date: Date

    var type: TransactionType {
        get { TransactionType(rawValue: typeRaw) ?? .expense }
        set { typeRaw = newValue.rawValue }
    }

    var expenseCategory: ExpenseCategory? {
        get { expenseCategoryRaw.flatMap(ExpenseCategory.init(rawValue:)) }
        set { expenseCategoryRaw = newValue?.rawValue }
    }

    var incomeCategory: IncomeCategory? {
        get { incomeCategoryRaw.flatMap(IncomeCategory.init(rawValue:)) }
        set { incomeCategoryRaw = newValue?.rawValue }
    }

    init(
        type: TransactionType,
        amount: Double,
        detail: String,
        expenseCategory: ExpenseCategory? = nil,
        incomeCategory: IncomeCategory? = nil,
        date: Date = .now
    ) {
        self.typeRaw = type.rawValue
        self.amount = amount
        self.detail = detail
        self.expenseCategoryRaw = expenseCategory?.rawValue
        self.incomeCategoryRaw = incomeCategory?.rawValue
        self.date = date
    }
}

enum class FarmFileKind: String, Codable, CaseIterable, Identifiable {
    case kmz, kml, geojson, csv, json, image, other
    var id: String { rawValue }
    var displayName: String { rawValue.uppercased() }
}

enum class ApiHttpMethod: String, Codable, CaseIterable, Identifiable {
    case get, post
    var id: String { rawValue }
    var displayName: String { rawValue.uppercased() }
}

@Model
final class FarmImportFile {
    var displayName: String
    var kindRaw: String
    var mimeType: String
    var storedPath: String
    var byteSize: Int64
    var notes: String
    var summary: String
    var importedAt: Date

    var kind: FarmFileKind {
        get { FarmFileKind(rawValue: kindRaw) ?? .other }
        set { kindRaw = newValue.rawValue }
    }

    init(
        displayName: String,
        kind: FarmFileKind,
        mimeType: String = "",
        storedPath: String,
        byteSize: Int64 = 0,
        notes: String = "",
        summary: String = "",
        importedAt: Date = .now
    ) {
        self.displayName = displayName
        self.kindRaw = kind.rawValue
        self.mimeType = mimeType
        self.storedPath = storedPath
        self.byteSize = byteSize
        self.notes = notes
        self.summary = summary
        self.importedAt = importedAt
    }
}

@Model
final class ApiFeedSource {
    var name: String
    var baseUrl: String
    var methodRaw: String
    var authHeader: String
    var notes: String
    var enabled: Bool
    var lastStatus: String
    var lastPulledAt: Date?
    var lastPreview: String
    var createdAt: Date

    var method: ApiHttpMethod {
        get { ApiHttpMethod(rawValue: methodRaw) ?? .get }
        set { methodRaw = newValue.rawValue }
    }

    init(
        name: String,
        baseUrl: String,
        method: ApiHttpMethod = .get,
        authHeader: String = "",
        notes: String = "",
        enabled: Bool = true,
        lastStatus: String = "",
        lastPulledAt: Date? = nil,
        lastPreview: String = "",
        createdAt: Date = .now
    ) {
        self.name = name
        self.baseUrl = baseUrl
        self.methodRaw = method.rawValue
        self.authHeader = authHeader
        self.notes = notes
        self.enabled = enabled
        self.lastStatus = lastStatus
        self.lastPulledAt = lastPulledAt
        self.lastPreview = lastPreview
        self.createdAt = createdAt
    }
}
