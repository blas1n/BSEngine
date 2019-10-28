#pragma once

#include "Interface.h"

INTERFACE_BEGIN(Manager)
	INTERFACE_DEF(bool, Init)
	INTERFACE_DEF(void, Update, float)
	INTERFACE_DEF(void, Release)
INTERFACE_END