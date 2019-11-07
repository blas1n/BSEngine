#pragma once

#include "IManager.h"

class BS_API MemoryManager final : public IManager
{
public:
	constexpr static size_t MEMORY_SIZE = 30000;

	MemoryManager() noexcept;

	bool Init() noexcept override;
	void Update(float deltaTime) noexcept override {}
	void Release() noexcept override;

	void* Allocate(size_t n) noexcept;
	void Deallocate(void* ptr, size_t n) noexcept;

private:
	class HeapMemory* memory;
};