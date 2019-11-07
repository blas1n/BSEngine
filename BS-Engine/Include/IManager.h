#pragma once

#include "Interface.h"
#include "Type.h"

/// @brief Interface defined for all managers to work with a consistent interface
INTERFACE_BEGIN(Manager)
	/// @brief Initialize manager.
	INTERFACE_DEF(bool, Init)

	/**
	 * @brief Update the manager.
	 * @param deltaTime Previous frame run time.
	*/
	INTERFACE_DEF(void, Update, float deltaTime)

	/// @brief Release the manager.
	INTERFACE_DEF(void, Release)
INTERFACE_END