//! The paged text screens reachable from the main menu: the story, the
//! instructions, the ordering information and the BBS advert.
//!
//! Generated from the `Show*` functions in game2.c rather than retyped, so
//! the wording is the game's own. Two things are deliberately dropped: the
//! cartoon panels those screens draw between the text, which are sprite
//! glyphs embedded in the strings, and the spinner that waits for a key.

/// One screen of a paged sequence. `top`/`height`/`width` are the
/// `UnfoldTextFrame` arguments; each line is (column offset from the
/// frame's text origin, absolute screen row, text).
pub struct TextPage {
    pub top: i32,
    pub height: i32,
    pub width: i32,
    pub title: &'static str,
    pub bottom: &'static str,
    pub lines: &'static [(i32, i32, &'static str)],
}

/// `ShowStory` (game2.c), transcribed.
pub const STORY: &[TextPage] = &[
    TextPage {
        top: 1, height: 23, width: 38,
        title: "STORY", bottom: "Press ANY key.",
        lines: &[
            (16, 5, "Tomorrow is Cosmo's"),
            (16, 7, "birthday, and his"),
            (16, 9, "parents are taking"),
            (16, 11, "him to the one place"),
            (16, 13, "in the Milky Way"),
            (16, 15, "galaxy that all kids"),
            (16, 17, "would love to go to:"),
            (16, 19, "   Disney World!"),
        ],
    },
    TextPage {
        top: 1, height: 23, width: 38,
        title: "STORY", bottom: "Press ANY key.",
        lines: &[
            (3, 5, "Suddenly a blazing comet zooms"),
            (4, 7, "toward their ship--leaving no"),
            (16, 10, "time"),
            (17, 12, "to"),
            (10, 15, "change course..."),
        ],
    },
    TextPage {
        top: 1, height: 23, width: 38,
        title: "STORY", bottom: "Press ANY key.",
        lines: &[
            (15, 7, "The comet slams into"),
            (1, 10, "the ship and forces Cosmo's"),
            (1, 13, "dad to make an"),
            (1, 15, "emergency landing"),
            (1, 17, "on an uncharted"),
            (1, 19, "planet."),
        ],
    },
    TextPage {
        top: 1, height: 23, width: 38,
        title: "STORY", bottom: "Press ANY key.",
        lines: &[
            (2, 5, "While Cosmo's"),
            (2, 7, "dad repairs"),
            (2, 9, "the ship,"),
            (11, 15, "Cosmo heads off to"),
            (11, 17, "explore and have"),
            (11, 19, "some fun."),
        ],
    },
    TextPage {
        top: 1, height: 23, width: 38,
        title: "STORY", bottom: "Press ANY key.",
        lines: &[
            (6, 7, "Returning an hour later,"),
            (17, 11, "Cosmo cannot find"),
            (17, 13, "his Mom or Dad."),
            (17, 15, "Instead, he finds"),
            (8, 18, "strange foot prints..."),
        ],
    },
    TextPage {
        top: 1, height: 23, width: 38,
        title: "STORY", bottom: "Press ANY key.",
        lines: &[
            (2, 5, "...oh no!  Has his"),
            (2, 7, "family been taken"),
            (2, 9, "away by a hungry"),
            (2, 11, "alien creature to"),
            (2, 13, "be eaten?  Cosmo"),
            (2, 15, "must rescue his"),
            (2, 17, "parents before"),
            (2, 19, "it's too late...!"),
        ],
    },
];

/// `ShowInstructions` (game2.c), transcribed.
pub const INSTRUCTIONS: &[TextPage] = &[
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Instructions  Page One of Five", bottom: "Press PgDn for next.  ESC to Exit.",
        lines: &[
            (0, 4, " OBJECT OF GAME:"),
            (0, 6, " On a strange and dangerous planet,"),
            (0, 8, " Cosmo must find and rescue his"),
            (0, 10, " parents."),
            (0, 13, " Cosmo, having seen big scary alien"),
            (0, 15, " footprints, believes his parents"),
            (0, 17, " have been captured and taken away"),
            (0, 19, " to be eaten!"),
        ],
    },
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Instructions  Page Two of Five", bottom: "Press PgUp or PgDn.  Esc to Exit.",
        lines: &[
            (0, 4, " Cosmo has a very special ability:"),
            (0, 6, " He can use his suction hands to"),
            (0, 8, " climb up walls."),
            (0, 11, " Warning:  Some surfaces, such as"),
            (0, 13, " ice, might be too slippery for"),
            (0, 15, " Cosmo to cling on firmly."),
        ],
    },
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Instructions  Page Three of Five", bottom: "Press PgUp or PgDn.  Esc to Exit.",
        lines: &[
            (0, 4, " Cosmo can jump onto attacking"),
            (0, 6, " creatures without being harmed."),
            (0, 8, " This is also Cosmo's way of"),
            (0, 10, " defending himself."),
            (0, 13, " Cosmo can also find and use bombs."),
        ],
    },
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Instructions  Page Four of Five", bottom: "Press PgUp or PgDn.  Esc to Exit.",
        lines: &[
            (0, 5, " Use the up and down arrow keys to"),
            (0, 7, " make Cosmo look up and down,"),
            (0, 9, " enabling him to see areas that"),
            (0, 11, " might be off the screen."),
            (0, 19, "      Up Key           Down Key"),
        ],
    },
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Instructions  Page Five of Five", bottom: "Press PgUp.  Esc to Exit.",
        lines: &[
            (0, 5, " In Cosmo's Cosmic Adventure, it's"),
            (0, 7, " up to you to discover the use of"),
            (0, 9, " all the neat and strange objects"),
            (0, 11, " you'll encounter on your journey."),
            (0, 13, " Secret Hint Globes will help"),
            (0, 15, " you along the way."),
        ],
    },
];

/// `ShowOrderingInformation` (game2.c), transcribed.
pub const ORDERING: &[TextPage] = &[
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Ordering Information", bottom: "Press ANY key.",
        lines: &[
            (0, 4, "      COSMO'S COSMIC ADVENTURE"),
            (0, 5, "    consists of three adventures."),
            (0, 7, "    Only the first adventure is"),
            (0, 8, " available as shareware.  The final"),
            (0, 9, "   two amazing adventures must be"),
            (0, 10, "    purchased from Apogee, or an"),
            (0, 11, "          authorized dealer."),
            (0, 13, "  The last two adventures of Cosmo"),
            (0, 14, "   feature exciting new graphics,"),
            (0, 15, "  new creatures, new puzzles, new"),
            (0, 16, "   music and all-new challenges!"),
            (0, 18, "    The next few screens provide"),
            (0, 19, "       ordering instructions."),
        ],
    },
    TextPage {
        top: 1, height: 22, width: 38,
        title: "Ordering Information", bottom: "Press ANY key.",
        lines: &[
            (0, 4, "       Order now and receive:"),
            (0, 6, "   * All three exciting adventures"),
            (0, 7, "   * The hints and tricks sheet"),
            (0, 8, "   * The Secret Cheat password"),
            (0, 9, "   * Exciting new bonus games"),
            (0, 11, "      To order, call toll free:"),
            (0, 12, "           1-800-426-3123"),
            (0, 13, "   (Visa and MasterCard Welcome)"),
            (0, 15, "   Order all three adventures for"),
            (0, 16, "     only $35, plus $4 shipping."),
        ],
    },
    TextPage {
        top: 1, height: 22, width: 38,
        title: "Ordering Information", bottom: "Press ANY key.",
        lines: &[
            (0, 4, "      Please specify disk size:"),
            (0, 5, "           5.25\\\"  or  3.5\\\""),
            (0, 7, "     To order send $35, plus $4"),
            (0, 8, "      shipping, USA funds, to:"),
            (0, 10, "           Apogee Software"),
            (0, 11, "           P.O. Box 476389"),
            (0, 12, "       Garland, TX 75047  (USA)"),
            (0, 14, "       Or CALL NOW toll free:"),
            (0, 15, "           1-800-426-3123"),
            (0, 18, "         ORDER COSMO TODAY!"),
            (0, 19, "           All 3 for $39!"),
        ],
    },
    TextPage {
        top: 4, height: 15, width: 38,
        title: "USE YOUR FAX MACHINE TO ORDER!", bottom: "Press ANY key.",
        lines: &[
            (0, 7, "  You can now use your FAX machine"),
            (0, 8, "   to order your favorite Apogee"),
            (0, 9, "     games quickly and easily."),
            (0, 11, "   Simply print out the ORDER.FRM"),
            (0, 12, "    file, fill it out and FAX it"),
            (0, 13, "    to us for prompt processing."),
            (0, 15, "     FAX Orders: (214) 278-4670"),
        ],
    },
    TextPage {
        top: 1, height: 20, width: 38,
        title: "About Apogee Software", bottom: "Press ANY key.",
        lines: &[
            (0, 4, "Our goal is to establish Apogee"),
            (0, 5, "  as the leader in commercial"),
            (0, 6, " quality shareware games. With"),
            (0, 7, " enthusiasm and dedication we"),
            (0, 8, "think our goal can be achieved."),
            (0, 10, "However,  we need your support."),
            (0, 11, "Shareware is not free software."),
            (0, 13, "  We thank you in advance for"),
            (0, 14, "   your contribution to the"),
            (0, 15, "  growing shareware community."),
        ],
    },
    TextPage {
        top: 0, height: 24, width: 38,
        title: "Ordering Information", bottom: "Press ANY key.",
        lines: &[
            (0, 4, "      COSMO'S COSMIC ADVENTURE"),
            (0, 6, "  This game IS commercial software."),
            (0, 8, "    This episode of Cosmo is NOT"),
            (0, 9, " available as shareware.  It is not"),
            (0, 10, "  freeware, nor public domain.  It"),
            (0, 11, "  is only available from Apogee or"),
            (0, 12, "        authorized dealers."),
            (0, 14, " If you are a registered player, we"),
            (0, 15, "    thank you for your patronage."),
            (0, 17, "  Please report any illegal selling"),
            (0, 18, "  and distribution of this game to"),
            (0, 19, "  Apogee by calling 1-800-GAME123."),
        ],
    },
];

/// `ShowPublisherBBS` (game2.c), transcribed.
pub const BBS: &[TextPage] = &[
    TextPage {
        top: 1, height: 22, width: 38,
        title: "THE OFFICIAL APOGEE BBS", bottom: "Press ANY key.",
        lines: &[
            (0, 3, "    -----------------------"),
            (0, 5, "The SOFTWARE CREATIONS BBS is"),
            (0, 6, " the home BBS for the latest"),
            (0, 7, " Apogee games.  Check out our"),
            (0, 8, "FREE 'Apogee' file section for"),
            (0, 9, "  new releases and updates."),
            (0, 11, "       BBS phone lines:"),
            (0, 13, "(508) 365-2359  2400 baud"),
            (0, 14, "(508) 365-9825  9600 baud"),
            (0, 15, "(508) 365-9668  14.4k dual HST"),
            (0, 17, "Home of the Apogee BBS Network!"),
            (0, 19, "    A Major Multi-Line BBS."),
        ],
    },
    TextPage {
        top: 0, height: 25, width: 40,
        title: "APOGEE ON AMERICA ONLINE!", bottom: "Press ANY key.",
        lines: &[
            (0, 2, "      -------------------------"),
            (0, 4, "   America Online (AOL) is host of"),
            (0, 5, " the Apogee Forum, where you can get"),
            (0, 6, "   new Apogee games. Use the Apogee"),
            (0, 7, "  message areas to talk and exchange"),
            (0, 8, "   ideas, comments and secrets with"),
            (0, 9, "   our designers and other players."),
            (0, 11, "  If you are already a member, after"),
            (0, 12, " you log on, use the keyword \\\"Apogee\\\""),
            (0, 13, " (Ctrl-K) to jump to the Apogee area."),
            (0, 15, "  If you'd like to know how to join,"),
            (0, 16, "        please call toll free:"),
            (0, 18, "            1-800-827-6364"),
            (0, 19, "    Please ask for extension 5703."),
            (0, 21, "   You'll get the FREE startup kit."),
        ],
    },
];
