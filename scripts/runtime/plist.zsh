# Persist every plist, but keep disabled services unloaded. No implicit enable command exists.
write_plist() {
  local label="$1" binary="$2" logname="$3" port="${4:-}"
  local full_label="com.linggan-intelligence.$label" disabled_flag
  local plist="$agents_dir/$full_label.plist"
  disabled_flag="$(python3 "$ai_guard" disabled-value "$disabled_snapshot" "$full_label")" || return 1
  python3 "$ai_guard" plist "$plist" "$full_label" "$binary" "$runtime_dir" \
    "$support_dir" "$log_dir" "$logname" "$drain_grace_seconds" "$port" || return 1
  plutil -lint "$plist" >/dev/null || return 1
  if [[ "$disabled_flag" == "true" ]]; then
    # Also records the new AI label's explicit initial off state.
    launchctl disable "gui/$(id -u)/$full_label" || return 1
    log "已更新 $full_label 配置；保持禁用，未启动"
  else
    launchctl bootstrap "gui/$(id -u)" "$plist" || return 1
    log "已更新并启动 $full_label"
  fi
}
