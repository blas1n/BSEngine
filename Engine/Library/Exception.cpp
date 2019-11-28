#include "Exception.h"
#include <cstdlib>
#include <utility>
#include "CString.h"

namespace BE
{
	Exception::Exception(Char* inMessage, MessageType type /*= MessageType::Deep*/) noexcept
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
		: message{ std::move(other.message) },
		needFree{ std::move(other.needFree) }
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
		message = std::move(other.message);
		needFree = std::move(other.needFree);
		other.needFree = false;
		return *this;
	}

	Exception::~Exception()
	{
		if (needFree)
			std::free(static_cast<void*>(message));
	}

	void Exception::Init(Char* inMessage)
	{
		if (!needFree)
		{
			message = inMessage;
			return;
		}
		
		const auto len = Strlen(inMessage);
		message = static_cast<Char*>(std::malloc((len + 1) * sizeof(Char)));
		Strcpy(message, 1, inMessage);
	}
}