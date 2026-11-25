#include <X11/XF86keysym.h>
/* See LICENSE file for copyright and license details. */

/* appearance */
static const unsigned int borderpx  = 1;        /* border pixel of windows */
static const unsigned int snap      = 32;       /* snap pixel */
static const unsigned int gappih    = 5;        /* horiz inner gap between windows */
static const unsigned int gappiv    = 5;        /* vert inner gap between windows */
static const unsigned int gappoh    = 5;        /* horiz outer gap between windows and screen edge */
static const unsigned int gappov    = 5;        /* vert outer gap between windows and screen edge */
static       int smartgaps          = 0;        /* 1 means no outer gap when there is only one window */
static const int showbar            = 1;        /* 0 means no bar */
static const int topbar             = 1;        /* 0 means bottom bar */
static const char *fonts[]          = { 
    "Maple Mono NL NF CN:size=12",
    "Symbols Nerd Font Mono:size=11:antialias=true:autohint=true" 
};
static const char dmenufont[]       = "Maple Mono NL NF CN:size=12";
static const unsigned int baralpha        = 0xb3;
static const unsigned int borderalpha     = 0x80;

/* Catppuccin Mocha Mauve Palette :3 */
static const char col_base[]        = "#1e1e2e"; /* Dark purplish background */
static const char col_surface[]     = "#45475a"; /* Soft muted border for unfocused windows */
static const char col_text[]        = "#cdd6f4"; /* Pastel whitish-purple text */
static const char col_crust[]       = "#11111b"; /* Very dark text for the selected bar */
static const char col_mauve[]       = "#cba6f7"; /* Purple selected background & borders */

static const char *colors[][3]      = {
    /*               fg         bg         border   */
    [SchemeNorm] = { col_text,  col_base,  col_surface },
    [SchemeSel]  = { col_crust, col_mauve, col_mauve   },
};

static const unsigned int alphas[][3]      = {
    /*               fg      bg        border     */
    [SchemeNorm] = { 0xff, baralpha, borderalpha },
    [SchemeSel]  = { 0xff, baralpha, borderalpha },
};

/* tagging */
static const char *tags[] = { "一", "二", "三", "四", "五", "六", "七", "八", "九" };

static const Rule rules[] = {
    /* xprop(1):
     *    WM_CLASS(STRING) = instance, class
     *    WM_NAME(STRING) = title
     */
    /* class          instance    title           tags mask     isfloating   monitor */
    { "Gimp-ex",      NULL,       NULL,           0,            1,           -1 },
    { "kitty-ranger", NULL,       "kitty-ranger", 0,            1,           -1 },
    { "Firefox-ex",   NULL,       NULL,           1 << 8,       0,           -1 },
};

/* layout(s) */
static const float mfact     = 0.5;  /* factor of master area size [0.05..0.95] */
static const int nmaster     = 1;    /* number of clients in master area */
static const int resizehints = 1;    /* 1 means respect size hints in tiled resizals */
static const int lockfullscreen = 1; /* 1 will force focus on the fullscreen window */
static const int refreshrate = 120;  /* refresh rate (per second) for client move/resize */

#define FORCE_VSPLIT 1  /* nrowgrid layout: force two clients to always split vertically */
#include "vanitygaps.c"

static const Layout layouts[] = {
    /* symbol     arrange function */
    { "󰕰",      tile },    /* first entry is default */
    { "󰊓",      monocle },
    { "󰔲",      spiral },
    { "[\\]",     dwindle },
    { "H[]",      deck },
    { "TTT",      bstack },
    { "===",      bstackhoriz },
    { "HHH",      grid },
    { "###",      nrowgrid },
    { "---",      horizgrid },
    { ":::",      gaplessgrid },
    { "|M|",      centeredmaster },
    { ">M>",      centeredfloatingmaster },
    { "",      NULL },    /* no layout function means floating behavior */
    { NULL,       NULL },
};

/* key definitions */
#define MODKEY Mod4Mask
#define TAGKEYS(KEY,TAG) \
    { MODKEY,                       KEY,      view,           {.ui = 1 << TAG} }, \
    { MODKEY|ControlMask,           KEY,      toggleview,     {.ui = 1 << TAG} }, \
    { MODKEY|ShiftMask,             KEY,      tag,            {.ui = 1 << TAG} }, \
    { MODKEY|ControlMask|ShiftMask, KEY,      toggletag,      {.ui = 1 << TAG} },

/* helper for spawning shell commands in the pre dwm-5.0 fashion */
#define SHCMD(cmd) { .v = (const char*[]){ "/bin/sh", "-c", cmd, NULL } }

#define STATUSBAR "dwmblocks"

/* commands */
static char dmenumon[2] = "0"; /* component of dmenucmd, manipulated in spawn() */
static const char *dmenucmd[] = { "/home/twfl/.config/rofi/launchers/type-1/launcher.sh", NULL };
static const char *powmenucmd[] = { "/home/twfl/.scripts/powermenu.sh", NULL };
static const char *termcmd[]  = { "kitty", NULL };
static const char *rangercmd[]  = { "kitty", "-e", "/home/twfl/.scripts/rangercd.sh", NULL };
static const char *lockcmd[] = { "slock", NULL };
static const char *screenshotcmd[] = { "sh", "-c", "maim -s | xclip -selection clipboard -t image/png", NULL };

/* audio commands */
static const char *upvol[]   = { "/home/twfl/.scripts/volume.sh", "up",   NULL };
static const char *downvol[] = { "/home/twfl/.scripts/volume.sh", "down", NULL };
static const char *mutevol[] = { "/home/twfl/.scripts/volume.sh", "mute", NULL };
static const char *mutemic[] = { "/home/twfl/.scripts/volume.sh", "mic",  NULL };

/* backlight commands */
static const char *brupcmd[]   = { "/home/twfl/.scripts/brightness.sh", "up", NULL };
static const char *brdowncmd[] = { "/home/twfl/.scripts/brightness.sh", "down", NULL };

static const Key keys[] = {
    /* modifier                     key        function        argument */
    { MODKEY,                       XK_r,      spawn,          {.v = dmenucmd } },
    { MODKEY,                       XK_Return, spawn,          {.v = termcmd } },
    { MODKEY,                       XK_e,      spawn,          {.v = rangercmd } },
    { MODKEY,                       XK_p,      spawn,          {.v = powmenucmd} },
    { MODKEY|Mod1Mask,              XK_l,      spawn,          {.v = lockcmd } },
    { MODKEY|ShiftMask,             XK_s,      spawn,          {.v = screenshotcmd } },
    { MODKEY,                       XK_b,      togglebar,      {0} },
    { MODKEY,                       XK_j,      focusstack,     {.i = +1 } },
    { MODKEY,                       XK_k,      focusstack,     {.i = -1 } },
    { MODKEY,                       XK_i,      incnmaster,     {.i = +1 } },
    { MODKEY,                       XK_d,      incnmaster,     {.i = -1 } },
    { MODKEY,                       XK_h,      setmfact,       {.f = -0.05} },
    { MODKEY,                       XK_l,      setmfact,       {.f = +0.05} },
    { MODKEY|ShiftMask,             XK_h,      setcfact,       {.f = +0.25} },
    { MODKEY|ShiftMask,             XK_l,      setcfact,       {.f = -0.25} },
    { MODKEY|ShiftMask,             XK_o,      setcfact,       {.f =  0.00} },
    { MODKEY|ShiftMask,             XK_Return, zoom,           {0} },
    { MODKEY|Mod1Mask,              XK_u,      incrgaps,       {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_u,      incrgaps,       {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_i,      incrigaps,      {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_i,      incrigaps,      {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_o,      incrogaps,      {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_o,      incrogaps,      {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_6,      incrihgaps,     {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_6,      incrihgaps,     {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_7,      incrivgaps,     {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_7,      incrivgaps,     {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_8,      incrohgaps,     {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_8,      incrohgaps,     {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_9,      incrovgaps,     {.i = +1 } },
    { MODKEY|Mod1Mask|ShiftMask,    XK_9,      incrovgaps,     {.i = -1 } },
    { MODKEY|Mod1Mask,              XK_0,      togglegaps,     {0} },
    { MODKEY|Mod1Mask|ShiftMask,    XK_0,      defaultgaps,    {0} },
    { MODKEY,                       XK_Tab,    view,           {0} },
    { MODKEY|ShiftMask,             XK_c,      killclient,     {0} },
    { MODKEY,                       XK_t,      setlayout,      {.v = &layouts[0]} }, /* tile */
    { MODKEY,                       XK_g,      setlayout,      {.v = &layouts[2]} }, /* spiral */
    { MODKEY,                       XK_m,      setlayout,      {.v = &layouts[1]} }, /* monocle */
    { MODKEY,                       XK_f,      setlayout,      {.v = &layouts[13]} }, /* floating */
    { MODKEY,                       XK_space,  setlayout,      {0} },
    { MODKEY|ShiftMask,             XK_space,  togglefloating, {0} },
    { MODKEY,                       XK_0,      view,           {.ui = ~0 } },
    { MODKEY|ShiftMask,             XK_0,      tag,            {.ui = ~0 } },
    { MODKEY,                       XK_comma,  focusmon,       {.i = -1 } },
    { MODKEY,                       XK_period, focusmon,       {.i = +1 } },
    { MODKEY|ShiftMask,             XK_comma,  tagmon,         {.i = -1 } },
    { MODKEY|ShiftMask,             XK_period, tagmon,         {.i = +1 } },
    { 0,                            XF86XK_AudioLowerVolume, spawn, {.v = downvol } },
    { 0,                            XF86XK_AudioMute,        spawn, {.v = mutevol } },
    { 0,                            XF86XK_AudioRaiseVolume, spawn, {.v = upvol   } },
    { 0,                            XF86XK_AudioMicMute,     spawn, {.v = mutemic } },
    { 0,                            XF86XK_MonBrightnessUp,   spawn, {.v = brupcmd } },
    { 0,                            XF86XK_MonBrightnessDown, spawn, {.v = brdowncmd } },
    TAGKEYS(                        XK_1,                      0)
    TAGKEYS(                        XK_2,                      1)
    TAGKEYS(                        XK_3,                      2)
    TAGKEYS(                        XK_4,                      3)
    TAGKEYS(                        XK_5,                      4)
    TAGKEYS(                        XK_6,                      5)
    TAGKEYS(                        XK_7,                      6)
    TAGKEYS(                        XK_8,                      7)
    TAGKEYS(                        XK_9,                      8)
    { MODKEY|ShiftMask,             XK_q,      quit,           {0} },
};

/* button definitions */
/* click can be ClkTagBar, ClkLtSymbol, ClkStatusText, ClkWinTitle, ClkClientWin, or ClkRootWin */
static const Button buttons[] = {
    /* click                event mask      button          function        argument */
    { ClkLtSymbol,          0,              Button1,        setlayout,      {0} },
    { ClkLtSymbol,          0,              Button3,        setlayout,      {.v = &layouts[2]} },
    { ClkWinTitle,          0,              Button2,        zoom,           {0} },
    { ClkStatusText,        0,              Button1,        sigstatusbar,   {.i = 1} },
    { ClkStatusText,        0,              Button2,        sigstatusbar,   {.i = 2} },
    { ClkStatusText,        0,              Button3,        sigstatusbar,   {.i = 3} },
    { ClkStatusText,        0,              Button4,        sigstatusbar,   {.i = 4} },
    { ClkStatusText,        0,              Button5,        sigstatusbar,   {.i = 5} },
    { ClkStatusText,        0,              6,              sigstatusbar,   {.i = 6} },
    { ClkStatusText,        0,              7,              sigstatusbar,   {.i = 7} },
    { ClkStatusText,        0,              8,              sigstatusbar,   {.i = 8} },
    { ClkStatusText,        0,              9,              sigstatusbar,   {.i = 9} },
    { ClkClientWin,         MODKEY,         Button1,        movemouse,      {0} },
    { ClkClientWin,         MODKEY,         Button2,        togglefloating, {0} },
    { ClkClientWin,         MODKEY,         Button3,        resizemouse,    {0} },
    { ClkTagBar,            0,              Button1,        view,           {0} },
    { ClkTagBar,            0,              Button3,        toggleview,     {0} },
    { ClkTagBar,            MODKEY,         Button1,        tag,            {0} },
    { ClkTagBar,            MODKEY,         Button3,        toggletag,      {0} },
};
