#include "System.h"
#include "MemoryManager.h"
#include "ThreadManager.h"

namespace BE
{
	static MemoryManager memoryManager;

	bool System::Initialize() noexcept {
		memoryManager.Init();
		MemoryManagerAccesser::Set(&memoryManager);
		
		Allocator<uint8> allocator;
		auto* const manager = static_cast<void*>(
				allocator.allocate(sizeof(ThreadManager)));

		// Allocate memory and call constructor each manager.
#pragma push_macro("new")
#undef new
		threadManager = new(manager)ThreadManager();
#pragma pop_macro("new")

		// Init and set accesser each manager.
		threadManager->Init();
		ThreadManagerAccesser::Set(threadManager);

		return true;
	}

	void System::RunLoop() noexcept {

	}

	void System::Release() noexcept {
		// Release each manager.
		threadManager->Release();

		memoryManager.Release();
	}
}