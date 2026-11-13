# Server Operations

Part of the OpenHome domain language; see [CONTEXT.md](../../CONTEXT.md).

**Server Tab**:
A mixed Top-Level Tab on `mobile-expo` that groups server-operations surfaces: AdGuard Protection control, the Docker Health Summary, and the Compact Timeline with Feed management.
_Avoid_: Infra dashboard, ops console

**Docker Tab**:
A Top-Level Tab on `mobile-expo` that lists Containers with their run state and exposes log inspection and lifecycle actions.
_Avoid_: Container manager, DevOps screen

**Container**:
One Docker workload reported by the Axum API with name, image, run state, and health status.
_Avoid_: Service, pod, stack

**Docker Health Summary**:
The client-side classification of all Containers into healthy, unhealthy, idle (none running), or offline (Axum API unreachable).
_Avoid_: Cluster status, uptime metric

**AdGuard Protection**:
The network-level ad blocking managed through the Axum API and controlled from the Server Tab.
_Avoid_: DNS filter, firewall setting

**Protection Pause**:
A time-boxed suspension of AdGuard Protection that resumes automatically when it ends.
_Avoid_: Disable, timeout

**Feed**:
A subscribed RSS source whose items appear in the Compact Timeline.
_Avoid_: Channel, subscription URL list

**Compact Timeline**:
The compact view of Feed items served by the Axum API that the Mobile Client paginates through newest-first.
_Avoid_: Article reader, full-content feed
