STAGING_DIR ?= ../../tmp-build

# Reuse dwm's own build settings when the staged source tree is available.
-include $(STAGING_DIR)/config.mk

.PHONY: test smoke-x11

test:
	$(CC) $(CPPFLAGS) $(CFLAGS) $(INCS) -I$(STAGING_DIR) verify_patches.c -o verify_patches $(LDFLAGS) $(LIBS)
	./verify_patches

smoke-x11:
	@set -eu; \
	Xvfb :99 -screen 0 1024x768x24 >/dev/null 2>&1 & \
	xvfb_pid=$$!; \
	trap 'kill $$xvfb_pid 2>/dev/null || true; wait $$xvfb_pid 2>/dev/null || true' EXIT HUP INT TERM; \
	sleep 1; \
	if ! kill -0 $$xvfb_pid 2>/dev/null; then \
		echo 'Xvfb failed to start' >&2; \
		exit 1; \
	fi; \
	set +e; DISPLAY=:99 timeout 2s $(STAGING_DIR)/dwm; status=$$?; set -e; \
	if [ $$status -eq 124 ]; then \
		echo 'dwm X11 smoke test passed'; \
		exit 0; \
	else \
		echo "dwm X11 smoke test failed with status $$status" >&2; \
		exit 1; \
	fi
