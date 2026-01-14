use reovim_core::declare_event_command;

declare_event_command! {
    HealthCheckOpen,
    id: "health_check_open",
    description: "Open health check modal",
}

declare_event_command! {
    HealthCheckClose,
    id: "health_check_close",
    description: "Close health check modal",
}

declare_event_command! {
    HealthCheckRerun,
    id: "health_check_rerun",
    description: "Re-run all health checks",
}

declare_event_command! {
    HealthCheckSelectNext,
    id: "health_check_select_next",
    description: "Select next check",
}

declare_event_command! {
    HealthCheckSelectPrev,
    id: "health_check_select_prev",
    description: "Select previous check",
}

declare_event_command! {
    HealthCheckToggle,
    id: "health_check_toggle",
    description: "Toggle expanded details for selected check",
}
