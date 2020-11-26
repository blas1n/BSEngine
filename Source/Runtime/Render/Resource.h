#pragma once

#include <string>

#define GENERATE_RESOURCE(name) \
public: \
	inline static const std::string& StaticClassName() noexcept \
	{ \
		static const std::string className{ #name }; \
		return className; \
	} \
\
	inline const std::string& ClassName() const noexcept override \
	{ \
		return name::StaticClassName(); \
	}

namespace ArenaBoss
{
	class Resource
	{
		GENERATE_RESOURCE(Resource);

	public:
		Resource(const std::string& inName)
			: name(inName) {}

		Resource(const Resource&) = delete;
		Resource(Resource&&) = delete;

		Resource& operator=(const Resource&) = delete;
		Resource& operator=(Resource&&) = delete;

		virtual ~Resource() = default;

		inline const std::string& GetName() const noexcept { return name; }

	private:
		friend bool operator==(const Resource& lhs, const Resource& rhs);
		friend bool operator<(const Resource& lhs, const Resource& rhs);

		friend bool operator==(const Resource& lhs, const std::string& rhs);
		friend bool operator<(const Resource& lhs, const std::string& rhs);

		friend bool operator==(const std::string& lhs, const Resource& rhs);
		friend bool operator<(const std::string& lhs, const Resource& rhs);

		std::string name;
	};

	inline bool operator==(const Resource& lhs, const Resource& rhs) { return lhs.name == rhs.name; }
	inline bool operator!=(const Resource& lhs, const Resource& rhs) { return !(lhs == rhs); }
	inline bool operator<(const Resource& lhs, const Resource& rhs) { return lhs.name < rhs.name; }
	inline bool operator>(const Resource& lhs, const Resource& rhs) { return rhs < lhs; }
	inline bool operator<=(const Resource& lhs, const Resource& rhs) { return !(rhs < lhs); }
	inline bool operator>=(const Resource& lhs, const Resource& rhs) { return !(lhs < rhs); }

	inline bool operator==(const Resource& lhs, const std::string& rhs) { return lhs.name == rhs; }
	inline bool operator!=(const Resource& lhs, const std::string& rhs) { return !(lhs == rhs); }
	inline bool operator<(const Resource& lhs, const std::string& rhs) { return lhs.name < rhs; }
	inline bool operator>(const Resource& lhs, const std::string& rhs) { return rhs < lhs; }
	inline bool operator<=(const Resource& lhs, const std::string& rhs) { return !(rhs < lhs); }
	inline bool operator>=(const Resource& lhs, const std::string& rhs) { return !(lhs < rhs); }

	inline bool operator==(const std::string& lhs, const Resource& rhs) { return lhs == rhs.name; }
	inline bool operator!=(const std::string& lhs, const Resource& rhs) { return !(lhs == rhs); }
	inline bool operator<(const std::string& lhs, const Resource& rhs) { return lhs < rhs.name; }
	inline bool operator>(const std::string& lhs, const Resource& rhs) { return rhs < lhs; }
	inline bool operator<=(const std::string& lhs, const Resource& rhs) { return !(rhs < lhs); }
	inline bool operator>=(const std::string& lhs, const Resource& rhs) { return !(lhs < rhs); }
}
