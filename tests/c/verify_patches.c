#include <stdio.h>
#include <stdlib.h>

/* dwm.c includes the staged drw.h and config.h after defining dwm's types. */
#define main dwm_original_main
#include "dwm.c"
#undef main

#if !defined(OPAQUE)
#error "01-alpha.diff: OPAQUE must be defined"
#elif OPAQUE != 0xffU
#error "01-alpha.diff: OPAQUE must equal 0xffU"
#endif

#define ARRAY_LEN(array) (sizeof(array) / sizeof((array)[0]))
#define STATIC_ASSERT(name, expression) typedef char static_assert_##name[(expression) ? 1 : -1]

STATIC_ASSERT(alpha_colors_are_nonempty, ARRAY_LEN(alphas) > 0);
STATIC_ASSERT(client_has_cfact, sizeof(((Client *)0)->cfact) == sizeof(float));
STATIC_ASSERT(monitor_has_gappih, sizeof(((Monitor *)0)->gappih) == sizeof(int));
STATIC_ASSERT(monitor_has_gappiv, sizeof(((Monitor *)0)->gappiv) == sizeof(int));
STATIC_ASSERT(monitor_has_gappoh, sizeof(((Monitor *)0)->gappoh) == sizeof(int));
STATIC_ASSERT(monitor_has_gappov, sizeof(((Monitor *)0)->gappov) == sizeof(int));
STATIC_ASSERT(status_width_symbol_exists, sizeof(statusw) > 0);
STATIC_ASSERT(status_signal_symbol_exists, sizeof(statussig) > 0);
STATIC_ASSERT(status_pid_symbol_exists, sizeof(statuspid) > 0);
STATIC_ASSERT(button_has_handler_field, sizeof(((Button *)0)->func) > 0);

static int
verify_alpha(void)
{
	if (ARRAY_LEN(alphas) == 0 || ARRAY_LEN(alphas[0]) == 0)
		return 0;
	return alphas[0][0] == OPAQUE;
}

static int
verify_cfacts(void)
{
	Arg arg = {.i = 7};
	void (*setcfact_symbol)(const Arg *) = setcfact;

	(void)setcfact_symbol;
	if (arg.i != 7)
		return 0;
	arg.f = 0.25f;
	return arg.f == 0.25f;
}

static int
verify_vanitygaps(void)
{
	volatile unsigned int gap_settings[] = {gappih, gappiv, gappoh, gappov};
	void (*toggle_symbol)(const Arg *) = togglegaps;
	void (*default_symbol)(const Arg *) = defaultgaps;

	(void)toggle_symbol;
	(void)default_symbol;
	return ARRAY_LEN(gap_settings) == 4;
}

static int
verify_statuscmd(void)
{
	size_t i;
	int status_buttons = 0;
	void (*signal_symbol)(const Arg *) = sigstatusbar;

	(void)signal_symbol;
	if (statusw < 0 || statussig < 0)
		return 0;

	for (i = 0; i < ARRAY_LEN(buttons); i++) {
		if (buttons[i].click == ClkStatusText) {
			if (buttons[i].func != sigstatusbar || buttons[i].arg.i < 1)
				return 0;
			status_buttons++;
		}
	}
	return status_buttons >= 5;
}

int
main(void)
{
	if (!verify_alpha() || !verify_cfacts()
	    || !verify_vanitygaps() || !verify_statuscmd()) {
		fputs("C patch assertion failed\n", stderr);
		return EXIT_FAILURE;
	}

	puts("ALL C PATCH ASSERTIONS PASSED");
	return EXIT_SUCCESS;
}
