import Foundation
import SwiftData

enum SeedData {
    static func ensureSeeded(context: ModelContext) {
        let profileDescriptor = FetchDescriptor<FarmProfile>()
        let profiles = (try? context.fetch(profileDescriptor)) ?? []
        if profiles.isEmpty {
            context.insert(FarmProfile(farmName: "Gut Farms"))
        }

        let animalDescriptor = FetchDescriptor<AnimalGroup>()
        let animals = (try? context.fetch(animalDescriptor)) ?? []
        guard animals.isEmpty else {
            try? context.save()
            return
        }

        let cattle = AnimalGroup(
            name: "Pasture Herd A",
            type: .cattle,
            count: 12,
            notes: "Mixed beef cattle",
            purchaseCost: 18000
        )
        let chickens = AnimalGroup(
            name: "Layer Coop 1",
            type: .chicken,
            count: 80,
            notes: "Rhode Island Reds",
            purchaseCost: 960
        )
        context.insert(cattle)
        context.insert(chickens)

        context.insert(
            FeedingSchedule(
                animalGroupName: cattle.name,
                feedName: "Hay + Grain mix",
                amountKg: 140,
                costPerKg: 0.28,
                frequency: .daily,
                timeOfDay: "07:00",
                notes: "Morning pasture top-up"
            )
        )
        context.insert(
            FeedingSchedule(
                animalGroupName: cattle.name,
                feedName: "Mineral lick check",
                amountKg: 2,
                costPerKg: 1.10,
                frequency: .daily,
                timeOfDay: "17:30"
            )
        )
        context.insert(
            FeedingSchedule(
                animalGroupName: chickens.name,
                feedName: "Layer pellets",
                amountKg: 10,
                costPerKg: 0.55,
                frequency: .twiceDaily,
                timeOfDay: "08:00"
            )
        )

        let now = Date.now
        let cattleBreed = Calendar.current.date(byAdding: .day, value: -30, to: now) ?? now
        let cattleDue = Calendar.current.date(byAdding: .day, value: AnimalType.cattle.gestationDays, to: cattleBreed) ?? now
        context.insert(
            BreedingSchedule(
                animalGroupName: cattle.name,
                animalType: .cattle,
                femaleLabel: "Cow #14",
                sireName: "Bull Ranger",
                method: .natural,
                status: .pregnant,
                breedingDate: cattleBreed,
                expectedDueDate: cattleDue,
                notes: "First calf for #14"
            )
        )

        let chickenBreed = Calendar.current.date(byAdding: .day, value: -5, to: now) ?? now
        let chickenDue = Calendar.current.date(byAdding: .day, value: AnimalType.chicken.gestationDays, to: chickenBreed) ?? now
        context.insert(
            BreedingSchedule(
                animalGroupName: chickens.name,
                animalType: .chicken,
                femaleLabel: "Broody hen group",
                sireName: "Rooster pen B",
                method: .natural,
                status: .dueSoon,
                breedingDate: chickenBreed,
                expectedDueDate: chickenDue,
                expectedOffspring: 12,
                notes: "Incubator tray 2"
            )
        )

        context.insert(
            AnimalArrival(
                name: "Maple",
                type: .cattle,
                origin: .purchased,
                eventDate: Calendar.current.date(byAdding: .day, value: -12, to: now) ?? now,
                registrationStatus: .registered,
                registrationId: "US-CA-4412",
                groupName: cattle.name,
                notes: "Bought at county sale"
            )
        )
        context.insert(
            AnimalArrival(
                name: "",
                type: .chicken,
                origin: .bornOnFarm,
                eventDate: Calendar.current.date(byAdding: .day, value: -2, to: now) ?? now,
                registrationStatus: .notRequired,
                groupName: chickens.name,
                notes: "Clutch from incubator tray 1"
            )
        )
        context.insert(
            AnimalArrival(
                name: "Pepper",
                type: .goat,
                origin: .transferredIn,
                eventDate: Calendar.current.date(byAdding: .day, value: -40, to: now) ?? now,
                registrationStatus: .pending,
                notes: "Awaiting herd book paperwork"
            )
        )

        context.insert(FarmTransaction(type: .income, amount: 420, detail: "Egg sales — weekly market", incomeCategory: .eggs))
        context.insert(FarmTransaction(type: .income, amount: 2400, detail: "Two steers sold", incomeCategory: .livestockSale))
        context.insert(FarmTransaction(type: .expense, amount: 310, detail: "Bulk feed delivery", expenseCategory: .feed))
        context.insert(FarmTransaction(type: .expense, amount: 150, detail: "Vet visit — herd check", expenseCategory: .veterinary))

        try? context.save()
    }
}
