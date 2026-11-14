//! Embedded host (M2): `libjdrserver.so`, loaded into the game process via a `DT_NEEDED` entry on
//! `libmain.so`; a constructor starts the server thread. Not built into the APK in Build 01.
