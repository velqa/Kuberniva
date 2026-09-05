<p align="center">
  <img src="public/kuberniva-mark.png" alt="Kuberniva logo" width="150">
</p>

<h1 align="center">Kuberniva</h1>

<p align="center">
  <strong>Kubernetes, in focus.</strong><br>
  A fast, native desktop workspace for people who operate many Kubernetes clusters.
</p>

<p align="center">
  <a href="https://github.com/onlysvijay-collab/Kuberniva/releases"><img src="https://img.shields.io/badge/version-0.3.18-1769aa?style=flat-square" alt="Version 0.3.18"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-2f855a?style=flat-square" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/Svelte%205-ff3e00?style=flat-square&logo=svelte&logoColor=white" alt="Svelte 5">
  <img src="https://img.shields.io/badge/Tauri%202-24c8db?style=flat-square&logo=tauri&logoColor=white" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Rust-native-ce422b?style=flat-square&logo=rust&logoColor=white" alt="Rust">
</p>

Kuberniva is an open-source, local-first Kubernetes desktop app. It keeps cluster switching quick and puts the information that matters—health, workloads, resources, events, logs, and safe operations—into one focused workspace.

## ✨ Highlights

| | What you can do |
| --- | --- |
| 🗂️ **Clusters** | Add kubeconfig files or folders, merge contexts, switch clusters from the top selector, and connect lazily only when a cluster is opened. |
| 🔐 **Authentication** | Use OIDC `exec`, OIDC auth-provider, bearer-token, and client-certificate kubeconfigs. |
| 🛡️ **Authorization** | Respect Kubernetes RBAC per cluster, namespace, API, object, verb, and subresource; hide unavailable APIs and keep read-only identities free of mutation controls. |
| ⭐ **Shortcuts** | Pin up to 10 clusters, rename their shortcuts, and keep them across restarts. |
| 📊 **Overview** | See cluster-wide CPU, memory, and node storage totals first, then select any node for its full capacity, allocation, network, and live usage details. |
| 🛎️ **Events** | Browse recent Kubernetes Events with warning/normal filters and search. |
| 🧭 **Resources** | Discover built-in configuration, access, network, Gateway API, storage, and cluster APIs; inspect metadata, labels, endpoint addresses, ports, and properties. Custom APIs have their own workspace. |
| 📝 **Editors** | Edit ConfigMaps and Secrets as key/value data, reveal Secret values from base64 on demand, edit YAML, save, and view certificate expiry. |
| 🚀 **Workloads** | Inspect Deployments, StatefulSets, DaemonSets, ReplicaSets, Jobs, CronJobs, Pods, and other discovered workload types. |
| 📜 **Operations** | Keep workload types visible while switching sibling-pod logs, select containers, refresh every 30 seconds, preserve scroll position, inspect Pod diagnostics, and open terminals with the active cluster context and namespace. |
| 🎨 **Workspace** | Use a full-window, light ivory/golden interface with a collapsible sidebar, persistent cluster context, loading states, confirmations, and actionable notifications. |

## 🏗️ Architecture

Kuberniva has two small layers:

- **Svelte 5 + TypeScript:** renders the workspace and owns navigation, selection, editors, sessions, and local preferences.
- **Tauri 2 + Rust:** parses kubeconfigs, runs OIDC authentication, discovers APIs and CRDs, talks to Kubernetes, reads logs, and executes context-bound commands.

```mermaid
flowchart LR
    UI["Svelte UI"] -->|Tauri invoke| HOST["Rust / Tauri host"]
    HOST --> CONFIG["Kubeconfig + OIDC"]
    CONFIG --> CLIENT["Cached kube client"]
    CLIENT --> DISCOVERY["Discovery + CRDs"]
    CLIENT --> API["Kubernetes APIs"]
    API --> FEATURES["Overview · Events · Resources · Custom APIs · Workloads · Logs · Terminal"]
    UI --> PREFS["Local workspace preferences"]
```

Kuberniva reads kubeconfig metadata locally, then creates and caches a Kubernetes client only for the selected context. Cluster sessions, selected namespaces, sidebar state, and favorite shortcut names are restored locally. No proxy service is required.

## 🚀 Run from source

Requirements: Node.js, npm, Rust, and macOS for the native desktop build.

~~~bash
npm install
npm run tauri dev
~~~

For a browser-only UI preview:

~~~bash
npm run dev
~~~

Live Kubernetes connections, OIDC execution, logs, and terminal access require the Tauri desktop host.

## 📦 Build and install locally

Build the Apple Silicon app bundle:

~~~bash
npm run tauri build -- --bundles app
~~~

The bundle is created at `src-tauri/target/release/bundle/macos/Kuberniva.app`. Copy it to the other Mac's `/Applications` folder.

<details>
<summary> macOS says the local app is damaged or from an unidentified developer?</summary>

For an ad-hoc local build, remove the quarantine attribute and open the app:

~~~bash
xattr -cr /Applications/Kuberniva.app
open /Applications/Kuberniva.app
~~~

</details>

Developer ID signing and notarization are required for a public distribution that opens without this first-launch step. Each user adds their own kubeconfig sources after installation.

## 🧪 Checks

~~~bash
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
~~~

## 📁 Repository layout

~~~text
public/                 # Logo and static assets
src/App.svelte          # Main Svelte workspace
src/app.css             # UI and responsive design system
src-tauri/src/lib.rs    # Rust/Tauri and Kubernetes integration
src-tauri/icons/        # Native app icons
LICENSE                 # MIT license
~~~

## License

Kuberniva is released under the [MIT License](LICENSE).
