//
//  ContentView.swift
//  FluxBarMobileProof
//
//  Simple status UI for the mobile runtime proof. The real evidence is produced
//  by the XCTest target; this view just proves the app launches and can call
//  the Rust core from the UI thread.
//

import SwiftUI

struct ContentView: View {
    @State private var status: String = "Tap to probe"

    var body: some View {
        VStack(spacing: 20) {
            Text("FluxBar Mobile Proof")
                .font(.title)
            Text(status)
                .font(.body)
                .multilineTextAlignment(.center)
                .padding()
            Button("Run runtime_info") {
                do {
                    let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
                        "probeAction": "runtime_info"
                    ])
                    status = "OK: \(response)"
                } catch {
                    status = "Error: \(error)"
                }
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }
}
