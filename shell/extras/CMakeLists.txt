# Version
add_executable(version version.cpp)
target_compile_definitions(version PRIVATE
    PROJECT_NAME="${PROJECT_NAME}"
    VERSION="${VERSION}"
    GIT_REVISION="${GIT_REVISION}"
    DISTRIBUTOR="${DISTRIBUTOR}"
)
install(TARGETS version DESTINATION ${INSTALL_LIBDIR})

find_package(Qt6 QUIET COMPONENTS Widgets)
find_package(KF6Config QUIET)
find_package(KF6Package QUIET)
find_package(KF6DBusAddons QUIET)
find_package(Plasma QUIET)
find_package(PlasmaQuick QUIET)

if(TARGET Qt6::Widgets AND TARGET KF6::ConfigCore AND TARGET KF6::Package AND TARGET Plasma::Plasma AND TARGET Plasma::PlasmaQuick)
    add_executable(plasma-applet-config plasma-applet-config.cpp)
    target_link_libraries(plasma-applet-config PRIVATE
        Qt6::Core
        Qt6::Gui
        Qt6::Widgets
        Qt6::Quick
        Qt6::Qml
        Plasma::Plasma
        Plasma::PlasmaQuick
        KF6::Package
        KF6::CoreAddons
        KF6::ConfigCore
        $<$<TARGET_EXISTS:KF6::DBusAddons>:KF6::DBusAddons>
    )
    if(TARGET KF6::DBusAddons)
        target_compile_definitions(plasma-applet-config PRIVATE HAVE_KDBUSADDONS)
    endif()
    install(TARGETS plasma-applet-config DESTINATION ${INSTALL_LIBDIR})
endif()


