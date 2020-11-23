#include "Exception.h"
#include <cstdlib>
#include <utility>
#include "CString.h"
#include "Utility.h"

namespace BE
{
	Exception::Exception(const Char* inMessage, MessageType type /*= MessageType::Deep*/) noexcept
		: message{ nullptr }, needFree{ type == MessageType::Deep }
	{
		Init(inMessage);
	}

	Exception::Exception(const Exception& other) noexcept
		: message{ nullptr }, needFree{ other.needFree }
	{
		Init(other.message);
	}

	Exception::Exception(Exception&& other) noexcept
		: message{ Move(other.message) },
		needFree{ Move(other.needFree) }
	{
		other.needFree = false;
	}

	Exception& Exception::operator=(const Exception& other) noexcept
	{
		needFree = other.needFree;
		Init(other.message);
		return *this;
	}

	Exception& Exception::operator=(Exception&& other) noexcept
	{
		message = Move(other.message);
		needFree = Move(other.needFree);
		other.needFree = false;
		return *this;
	}

	Exception::~Exception()
	{
		if (needFree)
			std::free(static_cast<void*>(const_cast<Char*>(message)));
	}

	void Exception::Init(const Char* inMessage)
	{
		if (!needFree)
		{
			message = inMessage;
			return;
		}
		
		const auto len = Strlen(inMessage);
		message = static_cast<const Char*>(std::malloc((len + 1) * sizeof(Char)));
		Strcpy(const_cast<Char*>(message), inMessage, len);
	}
}