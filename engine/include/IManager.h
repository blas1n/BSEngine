#pragma once

#include "Interface.h"
#include "Type.h"

/// @brief Interface defined for all managers to work with a consistent interface.
INTERFACE_BEGIN(Manager)
	/// @brief Initialize manager.
	INTERFACE_DEF(bool, Init)

	/// @brief Release the manager.
	INTERFACE_DEF(void, Release)
INTERFACE_END