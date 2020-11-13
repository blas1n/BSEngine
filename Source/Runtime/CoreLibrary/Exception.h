#pragma once

#include "Macro.h"
#include "Type.h"

namespace BE
{
	class BS_API Exception
	{
	public:
		enum class BS_API MessageType : Uint8
		{
			Shallow,
			Deep
		};

		constexpr Exception() noexcept
			: message{ TEXT("") }, needFree{ false } {}

		Exception(const Char* inMessage, MessageType type = MessageType::Deep) noexcept;

		Exception(const Exception& other) noexcept;
		Exception(Exception&& other) noexcept;

		Exception& operator=(const Exception& other) noexcept;
		Exception& operator=(Exception&& other) noexcept;

		virtual ~Exception();

		constexpr const Char* GetMessage() const noexcept { return message; }

	private:
		void Init(const Char* inMessage);

		const Char* message;
		bool needFree;
	};

	class RuntimeException : public Exception
	{
	public:
		using Exception::Exception;
	};

	class LogicException : public Exception
	{
	public:
		using Exception::Exception;
	};

	class InvalidArgumentException : public LogicException
	{
	public:
		using LogicException::LogicException;

		InvalidArgumentException()
			: LogicException{ TEXT("An invalid argument was passed."), MessageType::Shallow } {}
	};

	class OutOfRangeException : public LogicException {
	public:
		using LogicException::LogicException;

		OutOfRangeException()
			: LogicException{ TEXT("Array out of range."), MessageType::Shallow } {}
	};

	class OutOfMemoryException : public LogicException {
	public:
		using LogicException::LogicException;

		OutOfMemoryException()
			: LogicException{ TEXT("Insufficient memory."), MessageType::Shallow } {}
	};

	class BadAllocException : public RuntimeException {
	public:
		using RuntimeException::RuntimeException;

		BadAllocException()
			: RuntimeException{ TEXT("Can not allocate memory."), MessageType::Shallow } {}
	};

	class BadCastException : public RuntimeException {
	public:
		using RuntimeException::RuntimeException;

		BadCastException()
			: RuntimeException{ TEXT("Incorrect type casting."), MessageType::Shallow } {}
	};

	class SystemException : public RuntimeException {
	public:
		using RuntimeException::RuntimeException;

		SystemException()
			: RuntimeException{ TEXT("System internal error."), MessageType::Shallow } {}
	};

	class FileNotFoundException : public RuntimeException {
	public:
		using RuntimeException::RuntimeException;

		FileNotFoundException()
			: RuntimeException{ TEXT("The requested file could not be found."), MessageType::Shallow } {}
	};
}