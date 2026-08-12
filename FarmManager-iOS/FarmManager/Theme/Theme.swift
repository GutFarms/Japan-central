import SwiftUI

enum FarmTheme {
    static let forest = Color(red: 0.18, green: 0.36, blue: 0.23)
    static let forestDeep = Color(red: 0.12, green: 0.24, blue: 0.16)
    static let moss = Color(red: 0.42, green: 0.56, blue: 0.44)
    static let wheat = Color(red: 0.96, green: 0.79, blue: 0.37)
    static let cream = Color(red: 0.95, green: 0.97, blue: 0.94)
    static let mist = Color(red: 0.89, green: 0.93, blue: 0.89)
    static let softTeal = Color(red: 0.24, green: 0.48, blue: 0.42)
    static let softRed = Color(red: 0.72, green: 0.36, blue: 0.22)
    static let ink = Color(red: 0.10, green: 0.14, blue: 0.11)
}

struct ScreenHeader: View {
    let brand: String
    let title: String
    let subtitle: String
    var onBrandTap: (() -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(brand)
                .font(.system(.largeTitle, design: .serif).weight(.bold))
                .foregroundStyle(FarmTheme.wheat)
                .onTapGesture { onBrandTap?() }
            if onBrandTap != nil {
                Text("Tap name to rename farm")
                    .font(.footnote)
                    .foregroundStyle(FarmTheme.mist.opacity(0.9))
                    .onTapGesture { onBrandTap?() }
            }
            Text(title)
                .font(.title3.weight(.semibold))
                .foregroundStyle(FarmTheme.cream)
            Text(subtitle)
                .font(.subheadline)
                .foregroundStyle(FarmTheme.mist.opacity(0.92))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 20)
        .padding(.vertical, 28)
        .background(
            LinearGradient(
                colors: [FarmTheme.forestDeep, FarmTheme.forest, Color(red: 0.25, green: 0.48, blue: 0.30)],
                startPoint: .top,
                endPoint: .bottom
            )
        )
        .clipShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
    }
}

struct MetricTile: View {
    let label: String
    let value: String
    var accent: Color = FarmTheme.forest

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(.title, design: .serif).weight(.semibold))
                .foregroundStyle(accent)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Color.white)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
    }
}

/// Wheel-style picker for lists with more than five choices; compact menu otherwise.
struct ChoicePicker<T: Hashable & Identifiable>: View {
    let label: String
    let options: [T]
    @Binding var selection: T
    let title: (T) -> String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(.secondary)
            if options.count > 5 {
                Picker(label, selection: $selection) {
                    ForEach(options) { option in
                        Text(title(option)).tag(option)
                    }
                }
                .pickerStyle(.wheel)
                .frame(height: 160)
                .clipped()
                .background(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .fill(
                            LinearGradient(
                                colors: [FarmTheme.mist.opacity(0.55), .white, FarmTheme.mist.opacity(0.55)],
                                startPoint: .top,
                                endPoint: .bottom
                            )
                        )
                )
                .overlay(alignment: .center) {
                    Capsule()
                        .fill(FarmTheme.forest.opacity(0.12))
                        .frame(height: 36)
                        .padding(.horizontal, 10)
                        .allowsHitTesting(false)
                }
            } else {
                Picker(label, selection: $selection) {
                    ForEach(options) { option in
                        Text(title(option)).tag(option)
                    }
                }
                .pickerStyle(.menu)
            }
        }
    }
}

extension Double {
    var asCurrency: String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = "USD"
        return formatter.string(from: NSNumber(value: self)) ?? String(format: "$%.2f", self)
    }
}

extension Date {
    var mediumString: String {
        formatted(date: .abbreviated, time: .omitted)
    }
}

struct StringID: Hashable, Identifiable {
    let value: String
    var id: String { value }
    init(_ value: String) { self.value = value }
}
