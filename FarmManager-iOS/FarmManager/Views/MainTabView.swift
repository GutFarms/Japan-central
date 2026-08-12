import SwiftUI
import SwiftData

struct MainTabView: View {
    @Environment(\.modelContext) private var context
    @Query private var profiles: [FarmProfile]

    var farmName: String {
        profiles.first?.farmName ?? "Gut Farms"
    }

    var body: some View {
        TabView {
            HomeView(farmName: farmName)
                .tabItem { Label("Home", systemImage: "house") }
            LivestockView(farmName: farmName)
                .tabItem { Label("Herd", systemImage: "pawprint") }
            FeedingView(farmName: farmName)
                .tabItem { Label("Feed", systemImage: "leaf") }
            BreedingView(farmName: farmName)
                .tabItem { Label("Breed", systemImage: "heart") }
            ProfitsView(farmName: farmName)
                .tabItem { Label("Profit", systemImage: "chart.line.uptrend.xyaxis") }
        }
        .onAppear {
            SeedData.ensureSeeded(context: context)
        }
    }
}
