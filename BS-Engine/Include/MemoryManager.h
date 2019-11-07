#pragma once

#include "IManager.h"

/**
 * @brief Manager that manages all the memory used by the game engine.
 * @todo Build garbage collector.
*/
class BS_API MemoryManager final : public IManager
{
public:
	/**
	 * @brief Maximum amount of memory the game engine will use.
	 * @warning The game terminates when the amount of memory allocation becomes larger.
	*/
	constexpr static size_t MEMORY_SIZE = 30000;

	MemoryManager() noexcept;

	/// @brief Allocate memory for use by the game engine.
	bool Init() noexcept override;

	/// @breif It does nothing now, but exists for scalability.
	void Update(float deltaTime) noexcept override {}

	/// @brief Free the allocated memory.
	void Release() noexcept override;

	/**
	 * @brief Allocate memory.
	 * @param n Size to be allocated.
	 * @return Allocated pointer.
	 * @retval nullptr Can not allocate.
	*/
	void* Allocate(size_t n) noexcept;

	/**
	 * @brief Deallocate memory.
	 * @param ptr Pointer to be deallocated.
	 * @param n Size to be deallocated.
	*/
	void Deallocate(void* ptr, size_t n) noexcept;

private:
	class HeapMemory* memory;
};