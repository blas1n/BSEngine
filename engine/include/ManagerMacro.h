#pragma once

#include "Macro.h"

#define CREATE_MANAGER_ACCESSER(name) \
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
	static name * manager; \
	friend class System; \
};