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
                feedQuantity: 140,
                quantityUnit: .kg,
                costPerUnit: 0.28,
                animalsFed: 12,
                stockOnHand: 900,
                frequency: .daily,
                timeOfDay: "07:00",
                notes: "Morning pasture top-up"
            )
        )
        context.insert(
            FeedingSchedule(
                animalGroupName: cattle.name,
                feedName: "Mineral lick check",
                feedQuantity: 2,
                quantityUnit: .kg,
                costPerUnit: 1.10,
                animalsFed: 12,
                stockOnHand: 40,
                frequency: .daily,
                timeOfDay: "17:30"
            )
        )
        context.insert(
            FeedingSchedule(
                animalGroupName: chickens.name,
                feedName: "Layer pellets",
                feedQuantity: 10,
                quantityUnit: .kg,
                costPerUnit: 0.55,
                animalsFed: 80,
                stockOnHand: 200,
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

        context.insert(
            HealthRecord(
                animalGroupName: cattle.name,
                animalLabel: "Cow #14",
                type: .vaccination,
                title: "Clostridial booster",
                date: Calendar.current.date(byAdding: .day, value: -20, to: now) ?? now,
                provider: "Valley Vet",
                cost: 85,
                notes: "Annual herd round"
            )
        )
        context.insert(
            InventoryItem(
                name: "Layer pellets",
                category: .feed,
                quantity: 200,
                unit: "kg",
                reorderLevel: 50,
                unitCost: 0.55,
                location: "Feed shed"
            )
        )
        context.insert(
            InventoryItem(
                name: "Ivermectin",
                category: .medicine,
                quantity: 4,
                unit: "bottles",
                reorderLevel: 2,
                unitCost: 32,
                location: "Med cabinet"
            )
        )
        context.insert(
            JournalEntry(
                title: "Pasture rotation",
                category: .pasture,
                body: "Moved herd A to east paddock. Grass height good.",
                date: Calendar.current.date(byAdding: .day, value: -1, to: now) ?? now
            )
        )
        context.insert(
            FarmContact(
                name: "Dr. Helen Park",
                role: .veterinarian,
                phone: "555-0142",
                email: "helen@valleyvet.example",
                organization: "Valley Vet"
            )
        )
        context.insert(
            FarmContact(
                name: "Midwest Feed Co.",
                role: .supplier,
                phone: "555-0199",
                organization: "Midwest Feed"
            )
        )

        try? context.save()
    }
}
