#include "System.h"
#include "MemoryManager.h"
#include "ThreadManager.h"

namespace BE
{
	bool System::Initialize() noexcept {
		memoryManager =
			static_cast<MemoryManager*>(malloc(sizeof(MemoryManager)));

#pragma push_macro("new")
#undef new
		memoryManager = new(memoryManager)MemoryManager();

		return true;
	}

	void System::RunLoop() noexcept {

	}

	void System::Release() noexcept {

	}
}