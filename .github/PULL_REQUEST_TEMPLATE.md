## Summary

## Safety checklist

- [ ] No removal path bypasses `SafetyPolicy::validate`
- [ ] No filesystem path is accepted from the frontend for deletion
- [ ] `allowed_roots` / `ProtectedPaths` unchanged (or change is explained below)
- [ ] Tests use temporary directories only
- [ ] No network access, telemetry or background process added

## Testing
