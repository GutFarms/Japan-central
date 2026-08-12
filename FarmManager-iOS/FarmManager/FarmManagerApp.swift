import SwiftUI
import SwiftData

@main
struct FarmManagerApp: App {
    var sharedModelContainer: ModelContainer = {
        let schema = Schema([
            FarmProfile.self,
            AnimalGroup.self,
            FeedingSchedule.self,
            BreedingSchedule.self,
            AnimalArrival.self,
            FarmTransaction.self,
            FarmImportFile.self,
            ApiFeedSource.self
        ])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: false)
        do {
            return try ModelContainer(for: schema, configurations: [config])
        } catch {
            fatalError("Could not create ModelContainer: \(error)")
        }
    }()

    var body: some Scene {
        WindowGroup {
            MainTabView()
                .tint(FarmTheme.forest)
        }
        .modelContainer(sharedModelContainer)
    }
}
