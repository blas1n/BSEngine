#pragma once

#include "Macro.h"

/**
 * @brief A system that is a singleton object in the system layer.
 * @see README
*/
class BS_API System
{
public:
	/// @brief Initializes everything necessary to run the game engine.
	bool Initialize() noexcept;

	/// @brief To constantly call to update the scene with the manager.
	void RunLoop() noexcept;

	/// @brief Clean up everything created by the game engine.
	void Release() noexcept;

private:
	class MemoryManager* memoryManager;
	class ThreadManager* threadManager;
};