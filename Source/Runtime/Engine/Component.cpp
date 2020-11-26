#include "Component.h"
#include "JsonHelper.h"

namespace ArenaBoss
{
	void Component::Save(Json::JsonSaver& saver) const
	{
		Json::JsonHelper::AddString(saver, "type", StaticClassName());
	}
}