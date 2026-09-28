#ifdef PHOTON_LINUX_VULKAN_DIAGNOSTICS
#include <vulkan/vulkan.h>
#endif
#include <QDebug>
#include <QGuiApplication>
#include <QOpenGLContext>
#include <QOpenGLFunctions>
#include <QQmlApplicationEngine>
#include <QQmlError>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <atomic>
#include <cstdio>
#include <memory>
#ifdef PHOTON_LINUX_VULKAN_DIAGNOSTICS
#include <QVulkanInstance>
#include <cstring>
#endif

static void logQtQuickDevice(QQuickWindow *window) {
  auto *renderer = window->rendererInterface();
  auto api = renderer->graphicsApi();
  char const *api_name = "unknown";
  switch (api) {
  case QSGRendererInterface::Software:
    api_name = "software";
    break;
  case QSGRendererInterface::OpenGL:
    api_name = "OpenGL";
    break;
  case QSGRendererInterface::Vulkan:
    api_name = "Vulkan";
    break;
  case QSGRendererInterface::Direct3D11:
    api_name = "Direct3D 11";
    break;
  case QSGRendererInterface::Direct3D12:
    api_name = "Direct3D 12";
    break;
  case QSGRendererInterface::Metal:
    api_name = "Metal";
    break;
  case QSGRendererInterface::Null:
    api_name = "null";
    break;
  default:
    break;
  }
  std::fprintf(stderr, "Qt Quick graphics API: %s\n", api_name);
  if (api == QSGRendererInterface::OpenGL) {
    if (auto *context = QOpenGLContext::currentContext()) {
      auto *functions = context->functions();
      std::fprintf(
          stderr, "Qt Quick OpenGL GPU: %s vendor=%s version=%s\n",
          reinterpret_cast<char const *>(functions->glGetString(GL_RENDERER)),
          reinterpret_cast<char const *>(functions->glGetString(GL_VENDOR)),
          reinterpret_cast<char const *>(functions->glGetString(GL_VERSION)));
    } else {
      std::fprintf(stderr,
                   "Qt Quick OpenGL context unavailable for device logging\n");
    }
    return;
  }
  if (api != QSGRendererInterface::Vulkan)
    return;

#ifdef PHOTON_LINUX_VULKAN_DIAGNOSTICS
  auto *qt_instance = static_cast<QVulkanInstance *>(renderer->getResource(
      window, QSGRendererInterface::VulkanInstanceResource));
  auto *physical_device_resource =
      static_cast<VkPhysicalDevice *>(renderer->getResource(
          window, QSGRendererInterface::PhysicalDeviceResource));
  if (!qt_instance || !physical_device_resource) {
    std::fprintf(stderr, "Qt Quick Vulkan instance/device unavailable\n");
    return;
  }
  auto instance = qt_instance->vkInstance();
  auto physical_device = *physical_device_resource;
  auto get_properties = reinterpret_cast<PFN_vkGetPhysicalDeviceProperties2>(
      vkGetInstanceProcAddr(instance, "vkGetPhysicalDeviceProperties2"));
  if (!get_properties) {
    std::fprintf(stderr, "Qt Quick Vulkan device-property query unavailable\n");
    return;
  }
  VkPhysicalDeviceDriverProperties driver{
      .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DRIVER_PROPERTIES};
  VkPhysicalDeviceIDProperties identity{
      .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ID_PROPERTIES,
      .pNext = &driver};
  VkPhysicalDeviceProperties2 properties{
      .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
      .pNext = &identity};
  get_properties(physical_device, &properties);
  auto const &gpu = properties.properties;
  QByteArray uuid(reinterpret_cast<char const *>(identity.deviceUUID),
                  VK_UUID_SIZE);
  std::fprintf(stderr,
               "Qt Quick Vulkan GPU: %s vendor=0x%04x device=0x%04x "
               "driver=%s (%s) UUID=%s\n",
               gpu.deviceName, gpu.vendorID, gpu.deviceID, driver.driverName,
               driver.driverInfo, uuid.toHex().constData());
  auto *queue_family = static_cast<uint32_t *>(renderer->getResource(
      window, QSGRendererInterface::GraphicsQueueFamilyIndexResource));
  if (queue_family)
    std::fprintf(stderr, "Qt Quick Vulkan queue family: %u\n", *queue_family);
#else
  std::fprintf(stderr, "Qt Quick Vulkan device details unavailable\n");
#endif
}

int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  app.setApplicationName("Photon");
#ifdef Q_OS_LINUX
  QQuickWindow::setGraphicsApi(QSGRendererInterface::Vulkan);
#endif
  QQmlApplicationEngine engine;
  QObject::connect(&engine, &QQmlApplicationEngine::warnings, &app,
                   [](QList<QQmlError> const &warnings) {
                     for (auto const &warning : warnings)
                       qWarning().noquote() << warning.toString();
                   });
  engine.loadFromModule("Photon", "Main");
  if (engine.rootObjects().isEmpty())
    return 1;
  if (qEnvironmentVariableIsSet("PHOTON_VERBOSE")) {
    auto *window =
        qobject_cast<QQuickWindow *>(engine.rootObjects().constFirst());
    std::fprintf(stderr, "Qt Quick root window created: %s\n",
                 window ? "yes" : "no");
    std::fflush(stderr);
    if (window) {
      QObject::connect(
          window, &QQuickWindow::beforeRendering, window,
          [window, logged = std::make_shared<std::atomic_bool>(false)] {
            if (!logged->exchange(true, std::memory_order_relaxed))
              logQtQuickDevice(window);
          },
          Qt::DirectConnection);
    }
  }
  return app.exec();
}
