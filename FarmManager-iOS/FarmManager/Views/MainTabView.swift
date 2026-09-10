import SwiftUI
import SwiftData

private enum MainTab: String, CaseIterable, Identifiable {
    case home = "Home"
    case herd = "Herd"
    case feed = "Feed"
    case breed = "Breed"
    case profit = "Profit"

    var id: String { rawValue }

    var systemImage: String {
        switch self {
        case .home: return "house"
        case .herd: return "pawprint"
        case .feed: return "leaf"
        case .breed: return "heart"
        case .profit: return "chart.line.uptrend.xyaxis"
        }
    }
}

struct MainTabView: View {
    @Environment(\.modelContext) private var context
    @Query private var profiles: [FarmProfile]
    @State private var selected: MainTab = .home

    var farmName: String {
        profiles.first?.farmName ?? "Gut Farms"
    }

    var body: some View {
        VStack(spacing: 0) {
            // Primary section nav sits near the upper mid of the screen (below the status area).
            HStack(spacing: 0) {
                ForEach(MainTab.allCases) { tab in
                    Button {
                        selected = tab
                    } label: {
                        VStack(spacing: 4) {
                            Image(systemName: tab.systemImage)
                                .font(.system(size: 18, weight: .medium))
                            Text(tab.rawValue)
                                .font(.caption2.weight(.semibold))
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 10)
                        .foregroundStyle(selected == tab ? Color.accentColor : .secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .background(.bar)
            .overlay(alignment: .bottom) {
                Divider()
            }

            Group {
                switch selected {
                case .home:
                    HomeView(farmName: farmName)
                case .herd:
                    LivestockView(farmName: farmName)
                case .feed:
                    FeedingView(farmName: farmName)
                case .breed:
                    BreedingView(farmName: farmName)
                case .profit:
                    ProfitsView(farmName: farmName)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .onAppear {
            SeedData.ensureSeeded(context: context)
        }
    }
}
