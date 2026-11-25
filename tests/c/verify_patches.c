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
