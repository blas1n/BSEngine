function (register_library)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    
    file (GLOB_RECURSE SRCS "*.cpp")
    add_library (${MODULE_NAME} SHARED ${SRCS})
    target_include_directories (${MODULE_NAME} PUBLIC "Public")

    string (TOUPPER ${MODULE_NAME}_API API)
    target_compile_definitions (${MODULE_NAME} PRIVATE ${API}=${DLL_EXPORT} INTERFACE ${API}=${DLL_IMPORT})

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Public/pch.h")
        target_precompile_headers (${MODULE_NAME} PUBLIC Public/pch.h)
    endif ()

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Private/pch.h")
        target_precompile_headers (${MODULE_NAME} PRIVATE Private/pch.h)
    endif ()
endfunction ()

function (download_package NAME REPO TAG)
    set (SRC_DIR "${PROJECT_SOURCE_DIR}/Source/ThirdParty/${NAME}")
    set (BUILD_DIR "${CMAKE_BINARY_DIR}/ThirdParty/${NAME}-build")
    set (DOWNLOAD_DIR "${CMAKE_BINARY_DIR}/ThirdParty/${NAME}-download")

    message (${NAME}, ${REPO}, ${TAG}, ${SRC_DIR}, ${BUILD_DIR})

    configure_file ("${CMAKE_DIR}/AutoInstall.cmake" "${DOWNLOAD_DIR}/CMakeLists.txt" @ONLY)

    execute_process (
        COMMAND ${CMAKE_COMMAND} -G "${CMAKE_GENERATOR}" .
        RESULT_VARIABLE RESULT
        WORKING_DIRECTORY "${DOWNLOAD_DIR}"
    )

    if (RESULT)
        message (FATAL_ERROR "CMake step for ${NAME} failed: ${RESULT}")
    endif ()

    execute_process (
        COMMAND ${CMAKE_COMMAND} --build .
        RESULT_VARIABLE RESULT
        WORKING_DIRECTORY "${DOWNLOAD_DIR}"
    )

    if (RESULT)
        message (FATAL_ERROR "Build step for ${NAME} failed: ${RESULT}")
    endif ()

    if (EXISTS "${CMAKE_DIR}/${NAME}-build.cmake")
         configure_file ("${CMAKE_DIR}/${NAME}-build.cmake" "${SRC_DIR}/CMakeLists.txt" @ONLY)
    endif ()

    if (EXISTS "${SRC_DIR}/CMakeLists.txt")
         add_subdirectory ("${SRC_DIR}" "${BUILD_DIR}" EXCLUDE_FROM_ALL)
    endif ()
endfunction ()

function (get_package NAME REPO TAG)
    find_package (${NAME} CONFIG QUIET)

    if (${NAME}_FOUND)
        message (STATUS "Found ${NAME} from system")
    else ()
        message (STATUS "Could not find ${NAME} from system. Downloading...")
        download_package (${NAME} ${REPO} ${TAG})
    endif ()
endfunction ()
