#pragma once

#include "Macro.h"

#define DECLARE_MANAGER_ACCESSER(name) \
namespace BE \
{ \
	class name; \
	class BS_API name##Accesser \
	{ \
	protected: \
		inline static name * Get() noexcept \
		{ \
			check(manager != nullptr); \
			return manager; \
		} \
	\
	private: \
		inline static void Set(name * inManager) noexcept \
		{ \
			manager = inManager; \
		} \
	\
		static name* manager; \
		friend class System; \
	}; \
}

#define DEFINE_MANAGER_ACCESSER(name) \
namespace BE \
{ \
	name* name##Accesser::manager = nullptr; \
}