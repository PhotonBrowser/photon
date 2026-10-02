// Derived from the GPUI XPC presentation transport.
// Copyright 2022-2025 Zed Industries, Inc.
// Licensed under the Apache License, Version 2.0.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <mach/mach.h>
#import <xpc/xpc.h>

@protocol PhotonPresentationEventChannel
- (void)registerSharedEventHandle:(MTLSharedEventHandle *)handle
                       registryID:(uint64_t)registryID
                       forChannel:(NSString *)channel
                            reply:(void (^)(BOOL accepted))reply;
- (void)sharedEventHandleForChannel:(NSString *)channel
                              reply:(void (^)(MTLSharedEventHandle * _Nullable handle, uint64_t registryID))reply;
- (void)producerWaitForConsumerSubmissionOnChannel:(NSString *)channel frameID:(uint64_t)frameID
                                           reply:(void (^)(void))reply;
- (void)consumerSubmittedWaitOnChannel:(NSString *)channel frameID:(uint64_t)frameID
                                 reply:(void (^)(BOOL accepted))reply;
@end

static NSXPCInterface *PhotonPresentationInterface(void);

@interface PhotonPresentationEventBroker : NSObject <PhotonPresentationEventChannel>
@property(nonatomic, strong) NSMutableDictionary<NSString *, MTLSharedEventHandle *> *handles;
@property(nonatomic, strong) NSMutableDictionary<NSString *, NSNumber *> *registryIDs;
@property(nonatomic, strong) NSMutableDictionary<NSString *, dispatch_block_t> *producerWaiters;
@property(nonatomic, strong) NSMutableDictionary<NSString *, NSMutableArray *> *eventWaiters;
@end

@implementation PhotonPresentationEventBroker
- (instancetype)init
{
    if ((self = [super init])) {
        _handles = [NSMutableDictionary dictionary];
        _registryIDs = [NSMutableDictionary dictionary];
        _producerWaiters = [NSMutableDictionary dictionary];
        _eventWaiters = [NSMutableDictionary dictionary];
    }
    return self;
}

- (void)registerSharedEventHandle:(MTLSharedEventHandle *)handle
                       registryID:(uint64_t)registryID
                       forChannel:(NSString *)channel
                            reply:(void (^)(BOOL))reply
{
    NSArray *waiters = nil;
    @synchronized(self) {
        self.handles[channel] = handle;
        self.registryIDs[channel] = @(registryID);
        waiters = self.eventWaiters[channel];
        [self.eventWaiters removeObjectForKey:channel];
    }
    reply(YES);
    for (void (^waiter)(MTLSharedEventHandle *, uint64_t) in waiters)
        waiter(handle, registryID);
}

- (void)sharedEventHandleForChannel:(NSString *)channel
                              reply:(void (^)(MTLSharedEventHandle * _Nullable, uint64_t))reply
{
    MTLSharedEventHandle *handle = nil;
    uint64_t registryID = 0;
    BOOL pending = NO;
    @synchronized(self) {
        handle = self.handles[channel];
        registryID = self.registryIDs[channel].unsignedLongLongValue;
        if (!handle) {
            NSMutableArray *waiters = self.eventWaiters[channel];
            if (!waiters) {
                waiters = [NSMutableArray array];
                self.eventWaiters[channel] = waiters;
            }
            [waiters addObject:[reply copy]];
            pending = YES;
        }
    }
    if (!pending)
        reply(handle, registryID);
}

- (void)producerWaitForConsumerSubmissionOnChannel:(NSString *)channel frameID:(uint64_t)frameID reply:(void (^)(void))reply
{
    NSString *key = [NSString stringWithFormat:@"%@/%llu", channel, frameID];
    @synchronized(self) {
        self.producerWaiters[key] = [reply copy];
    }
}

- (void)consumerSubmittedWaitOnChannel:(NSString *)channel frameID:(uint64_t)frameID reply:(void (^)(BOOL))reply
{
    NSString *key = [NSString stringWithFormat:@"%@/%llu", channel, frameID];
    dispatch_block_t producerReply = nil;
    @synchronized(self) {
        producerReply = self.producerWaiters[key];
        [self.producerWaiters removeObjectForKey:key];
    }
    if (!producerReply) {
        reply(NO);
        return;
    }
    producerReply();
    reply(YES);
}

@end

@interface PhotonPresentationEventListenerDelegate : NSObject <NSXPCListenerDelegate>
@property(nonatomic, strong) PhotonPresentationEventBroker *broker;
@end

@implementation PhotonPresentationEventListenerDelegate
- (instancetype)init
{
    if ((self = [super init]))
        _broker = [[PhotonPresentationEventBroker alloc] init];
    return self;
}

- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)connection
{
    connection.exportedInterface = PhotonPresentationInterface();
    connection.exportedObject = self.broker;
    [connection resume];
    return YES;
}
@end

typedef struct {
    __strong NSXPCConnection *object_connection;
    xpc_connection_t surface_connection;
} PhotonPresentationConnection;

static NSMutableDictionary<NSString *, id> *PhotonSurfacePorts;
static xpc_connection_t PhotonSurfaceListener;
static NSMutableSet<NSString *> *PhotonReleasedFrames;
static NSMutableSet<NSString *> *PhotonOutstandingFrames;
static NSMutableDictionary<NSString *, NSNumber *> *PhotonLastSignalValues;
static NSMutableDictionary<NSString *, NSMutableArray<NSDictionary *> *> *PhotonReleaseWaiters;
static NSMutableDictionary<NSString *, NSDictionary *> *PhotonPendingFrames;
static NSMutableDictionary<NSString *, NSMutableArray<NSDictionary *> *> *PhotonFrameWaiters;

static NSString *PhotonBackingKey(NSString *channel, uint64_t backing, uint64_t generation)
{
    return [NSString stringWithFormat:@"%@/%llu/%llu", channel, backing, generation];
}

static NSString *PhotonFrameKey(NSString *channel, uint64_t backing, uint64_t generation, uint64_t frame)
{
    return [NSString stringWithFormat:@"%@/%llu/%llu/%llu", channel, backing, generation, frame];
}

static void PhotonStartSurfaceService(NSString *name)
{
    PhotonSurfacePorts = [NSMutableDictionary dictionary];
    PhotonReleasedFrames = [NSMutableSet set];
    PhotonOutstandingFrames = [NSMutableSet set];
    PhotonLastSignalValues = [NSMutableDictionary dictionary];
    PhotonReleaseWaiters = [NSMutableDictionary dictionary];
    PhotonPendingFrames = [NSMutableDictionary dictionary];
    PhotonFrameWaiters = [NSMutableDictionary dictionary];
    PhotonSurfaceListener = xpc_connection_create_mach_service(name.UTF8String, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), XPC_CONNECTION_MACH_SERVICE_LISTENER);
    xpc_connection_set_event_handler(PhotonSurfaceListener, ^(xpc_object_t peer) {
        if (xpc_get_type(peer) != XPC_TYPE_CONNECTION)
            return;
        xpc_connection_set_event_handler(peer, ^(xpc_object_t request) {
            if (xpc_get_type(request) != XPC_TYPE_DICTIONARY)
                return;
            const char *operation = xpc_dictionary_get_string(request, "op");
            const char *channel_c = xpc_dictionary_get_string(request, "channel");
            uint64_t backing = xpc_dictionary_get_uint64(request, "backing");
            uint64_t generation = xpc_dictionary_get_uint64(request, "generation");
            uint64_t frame = xpc_dictionary_get_uint64(request, "frame");
            if (!operation || !channel_c)
                return;
            NSString *channel = [NSString stringWithUTF8String:channel_c];
            NSString *key = PhotonBackingKey(channel, backing, generation);
            xpc_object_t reply = xpc_dictionary_create_reply(request);
            if (strcmp(operation, "ping") == 0) {
                xpc_dictionary_set_string(reply, "reply", "pong");
            } else if (strcmp(operation, "register") == 0) {
                mach_port_t port = xpc_dictionary_copy_mach_send(request, "port");
                BOOL accepted = port != MACH_PORT_NULL;
                if (accepted) {
                    @synchronized(PhotonSurfacePorts) {
                        PhotonSurfacePorts[key] = @{
                            @"port": @(port),
                            @"width": @(xpc_dictionary_get_uint64(request, "width")),
                            @"height": @(xpc_dictionary_get_uint64(request, "height")),
                            @"pixel_format": @(xpc_dictionary_get_uint64(request, "pixel_format")),
                        };
                    }
                    xpc_dictionary_set_bool(reply, "accepted", true);
                    xpc_dictionary_set_uint64(reply, "width", xpc_dictionary_get_uint64(request, "width"));
                    xpc_dictionary_set_uint64(reply, "height", xpc_dictionary_get_uint64(request, "height"));
                    xpc_dictionary_set_uint64(reply, "pixel_format", xpc_dictionary_get_uint64(request, "pixel_format"));
                } else {
                    xpc_dictionary_set_bool(reply, "accepted", false);
                }
            } else if (strcmp(operation, "copy") == 0) {
                mach_port_t port = MACH_PORT_NULL;
                uint64_t width = 0, height = 0, pixel_format = 0;
                @synchronized(PhotonSurfacePorts) {
                    NSDictionary *entry = PhotonSurfacePorts[key];
                    if (entry) {
                        port = [entry[@"port"] unsignedIntValue];
                        width = [entry[@"width"] unsignedLongLongValue];
                        height = [entry[@"height"] unsignedLongLongValue];
                        pixel_format = [entry[@"pixel_format"] unsignedLongLongValue];
                    }
                }
                if (port != MACH_PORT_NULL) {
                    xpc_dictionary_set_mach_send(reply, "port", port);
                    xpc_dictionary_set_uint64(reply, "width", width);
                    xpc_dictionary_set_uint64(reply, "height", height);
                    xpc_dictionary_set_uint64(reply, "pixel_format", pixel_format);
                    xpc_dictionary_set_bool(reply, "accepted", true);
                } else {
                    xpc_dictionary_set_bool(reply, "accepted", false);
                }
            } else if (strcmp(operation, "unregister") == 0) {
                mach_port_t port = MACH_PORT_NULL;
                BOOL canRetire = YES;
                NSString *prefix = [key stringByAppendingString:@"/"];
                @synchronized(PhotonSurfacePorts) {
                    for (NSString *leased in PhotonOutstandingFrames) {
                        if ([leased hasPrefix:prefix]) {
                            canRetire = NO;
                            break;
                        }
                    }
                    NSDictionary *entry = PhotonSurfacePorts[key];
                    if (canRetire && entry) {
                        port = [entry[@"port"] unsignedIntValue];
                        [PhotonSurfacePorts removeObjectForKey:key];
                    }
                }
                if (port != MACH_PORT_NULL)
                    mach_port_deallocate(mach_task_self(), port);
                xpc_dictionary_set_bool(reply, "accepted", canRetire && port != MACH_PORT_NULL);
            } else if (strcmp(operation, "publish_frame") == 0 || strcmp(operation, "wait_frame") == 0) {
                NSDictionary *frame_descriptor = nil;
                NSArray<NSDictionary *> *waiters = nil;
                BOOL backingRegistered = NO;
                @synchronized(PhotonSurfacePorts) {
                    backingRegistered = PhotonSurfacePorts[key] != nil;
                    if (strcmp(operation, "publish_frame") == 0 && backingRegistered) {
                        NSString *frame_key = PhotonFrameKey(channel, backing, generation, frame);
                        uint64_t signal_value = xpc_dictionary_get_uint64(request, "signal_value");
                        uint64_t previous = [PhotonLastSignalValues[channel] unsignedLongLongValue];
                        if (signal_value == 0 || signal_value <= previous || [PhotonReleasedFrames containsObject:frame_key] || [PhotonOutstandingFrames containsObject:frame_key]) {
                            backingRegistered = NO;
                        } else {
                            PhotonLastSignalValues[channel] = @(signal_value);
                            [PhotonOutstandingFrames addObject:frame_key];
                            frame_descriptor = @{
                                @"backing": @(backing),
                                @"generation": @(generation),
                                @"frame": @(frame),
                                @"signal_value": @(signal_value),
                            };
                            waiters = PhotonFrameWaiters[channel];
                            [PhotonFrameWaiters removeObjectForKey:channel];
                            if (waiters.count == 0)
                                PhotonPendingFrames[channel] = frame_descriptor;
                        }
                    } else if (strcmp(operation, "wait_frame") == 0) {
                        frame_descriptor = PhotonPendingFrames[channel];
                        if (frame_descriptor) {
                            [PhotonPendingFrames removeObjectForKey:channel];
                        } else {
                            NSMutableArray *pending = PhotonFrameWaiters[channel];
                            if (!pending) {
                                pending = [NSMutableArray array];
                                PhotonFrameWaiters[channel] = pending;
                            }
                            [pending addObject:@{@"peer": peer, @"reply": reply}];
                        }
                    }
                }
                if (strcmp(operation, "publish_frame") == 0) {
                    xpc_dictionary_set_bool(reply, "accepted", backingRegistered);
                    xpc_connection_send_message(peer, reply);
                    for (NSDictionary *waiter in waiters) {
                        xpc_object_t waiting_reply = (xpc_object_t)waiter[@"reply"];
                        xpc_connection_t waiting_peer = (xpc_connection_t)waiter[@"peer"];
                        xpc_dictionary_set_bool(waiting_reply, "accepted", true);
                        xpc_dictionary_set_uint64(waiting_reply, "backing", [frame_descriptor[@"backing"] unsignedLongLongValue]);
                        xpc_dictionary_set_uint64(waiting_reply, "generation", [frame_descriptor[@"generation"] unsignedLongLongValue]);
                        xpc_dictionary_set_uint64(waiting_reply, "frame", [frame_descriptor[@"frame"] unsignedLongLongValue]);
                        xpc_dictionary_set_uint64(waiting_reply, "signal_value", [frame_descriptor[@"signal_value"] unsignedLongLongValue]);
                        xpc_connection_send_message(waiting_peer, waiting_reply);
                    }
                    return;
                }
                if (frame_descriptor) {
                    xpc_dictionary_set_bool(reply, "accepted", true);
                    xpc_dictionary_set_uint64(reply, "backing", [frame_descriptor[@"backing"] unsignedLongLongValue]);
                    xpc_dictionary_set_uint64(reply, "generation", [frame_descriptor[@"generation"] unsignedLongLongValue]);
                    xpc_dictionary_set_uint64(reply, "frame", [frame_descriptor[@"frame"] unsignedLongLongValue]);
                    xpc_dictionary_set_uint64(reply, "signal_value", [frame_descriptor[@"signal_value"] unsignedLongLongValue]);
                    xpc_connection_send_message(peer, reply);
                }
                return;
            } else if (strcmp(operation, "release") == 0 || strcmp(operation, "wait_release") == 0) {
                NSString *frameKey = PhotonFrameKey(channel, backing, generation, frame);
                BOOL backingRegistered = NO;
                NSArray<NSDictionary *> *waiters = nil;
                BOOL alreadyReleased = NO;
                BOOL outstanding = NO;
                @synchronized(PhotonSurfacePorts) {
                    backingRegistered = PhotonSurfacePorts[key] != nil;
                    alreadyReleased = [PhotonReleasedFrames containsObject:frameKey];
                    outstanding = [PhotonOutstandingFrames containsObject:frameKey];
                    if (strcmp(operation, "release") == 0 && backingRegistered && !alreadyReleased && outstanding) {
                        [PhotonReleasedFrames addObject:frameKey];
                        [PhotonOutstandingFrames removeObject:frameKey];
                        waiters = PhotonReleaseWaiters[frameKey];
                        [PhotonReleaseWaiters removeObjectForKey:frameKey];
                    }
                    if (strcmp(operation, "wait_release") == 0 && backingRegistered && outstanding && !alreadyReleased) {
                        NSMutableArray *pending = PhotonReleaseWaiters[frameKey];
                        if (!pending) {
                            pending = [NSMutableArray array];
                            PhotonReleaseWaiters[frameKey] = pending;
                        }
                        [pending addObject:@{@"peer": peer, @"reply": reply}];
                    }
                }
                if (strcmp(operation, "release") == 0) {
                    xpc_dictionary_set_bool(reply, "accepted", backingRegistered && outstanding && !alreadyReleased);
                    xpc_connection_send_message(peer, reply);
                    for (NSDictionary *waiter in waiters) {
                        xpc_object_t waiting_reply = (xpc_object_t)waiter[@"reply"];
                        xpc_connection_t waiting_peer = (xpc_connection_t)waiter[@"peer"];
                        xpc_dictionary_set_bool(waiting_reply, "accepted", true);
                        xpc_connection_send_message(waiting_peer, waiting_reply);
                    }
                    return;
                }
                if (alreadyReleased || !backingRegistered || !outstanding) {
                    xpc_dictionary_set_bool(reply, "accepted", alreadyReleased);
                    xpc_connection_send_message(peer, reply);
                }
                return;
            }
            xpc_connection_send_message(peer, reply);
        });
        xpc_connection_resume(peer);
    });
    xpc_connection_resume(PhotonSurfaceListener);
}

static NSString *PhotonString(const char *value)
{
    if (!value)
        return nil;
    return [[NSString alloc] initWithUTF8String:value];
}

static NSXPCInterface *PhotonPresentationInterface(void)
{
    NSXPCInterface *interface = [NSXPCInterface interfaceWithProtocol:@protocol(PhotonPresentationEventChannel)];
    [interface setClasses:[NSSet setWithObject:[MTLSharedEventHandle class]]
               forSelector:@selector(registerSharedEventHandle:registryID:forChannel:reply:)
             argumentIndex:0
                   ofReply:NO];
    [interface setClasses:[NSSet setWithObject:[MTLSharedEventHandle class]]
               forSelector:@selector(sharedEventHandleForChannel:reply:)
             argumentIndex:0
                   ofReply:YES];
    return interface;
}

void *photon_presentation_xpc_connect(const char *service_name)
{
    @autoreleasepool {
        NSString *name = PhotonString(service_name);
        if (!name)
            return NULL;
        NSXPCConnection *connection = [[NSXPCConnection alloc] initWithMachServiceName:name options:0];
        connection.remoteObjectInterface = PhotonPresentationInterface();
        [connection resume];
        NSString *surfaceName = [name stringByAppendingString:@".iosurface"];
        xpc_connection_t surface = xpc_connection_create_mach_service(surfaceName.UTF8String, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), 0);
        if (!surface) {
            [connection invalidate];
            return NULL;
        }
        xpc_connection_set_event_handler(surface, ^(xpc_object_t event) {
            (void)event;
        });
        xpc_connection_resume(surface);
        xpc_object_t ping = xpc_dictionary_create(NULL, NULL, 0);
        xpc_dictionary_set_string(ping, "op", "ping");
        xpc_dictionary_set_string(ping, "channel", "__bootstrap_ping__");
        xpc_object_t pong = xpc_connection_send_message_with_reply_sync(surface, ping);
        const char *reply = pong && xpc_get_type(pong) == XPC_TYPE_DICTIONARY ? xpc_dictionary_get_string(pong, "reply") : NULL;
        BOOL pingSucceeded = reply && strcmp(reply, "pong") == 0;
        if (!pingSucceeded) {
            [connection invalidate];
            xpc_connection_cancel(surface);
            return NULL;
        }
        PhotonPresentationConnection *pair = calloc(1, sizeof(PhotonPresentationConnection));
        pair->object_connection = connection;
        pair->surface_connection = surface;
        return pair;
    }
}

void photon_presentation_xpc_disconnect(void *opaque_connection)
{
    if (!opaque_connection)
        return;
    PhotonPresentationConnection *pair = opaque_connection;
    [pair->object_connection invalidate];
    xpc_connection_cancel(pair->surface_connection);
    free(pair);
}

bool photon_presentation_xpc_register_event(void *opaque_connection, const char *channel_name, uint64_t registry_id, void *opaque_event)
{
    if (!opaque_connection || !opaque_event)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;

    NSXPCConnection *connection = ((PhotonPresentationConnection *)opaque_connection)->object_connection;
    id<PhotonPresentationEventChannel> proxy = (id<PhotonPresentationEventChannel>)[connection remoteObjectProxyWithErrorHandler:^(__unused NSError *error) {}];
    id<MTLSharedEvent> event = (__bridge id<MTLSharedEvent>)opaque_event;
    MTLSharedEventHandle *handle = [event newSharedEventHandle];
    if (!handle)
        return false;

    dispatch_semaphore_t completed = dispatch_semaphore_create(0);
    __block BOOL accepted = NO;
    [proxy registerSharedEventHandle:handle registryID:registry_id forChannel:channel reply:^(BOOL result) {
        accepted = result;
        dispatch_semaphore_signal(completed);
    }];
    if (dispatch_semaphore_wait(completed, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC)) != 0)
        return false;
    return accepted;
}

void *photon_presentation_xpc_copy_event_handle(void *opaque_connection, const char *channel_name, uint64_t *registry_id)
{
    if (!opaque_connection)
        return NULL;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return NULL;

    NSXPCConnection *connection = ((PhotonPresentationConnection *)opaque_connection)->object_connection;
    id<PhotonPresentationEventChannel> proxy = (id<PhotonPresentationEventChannel>)[connection remoteObjectProxyWithErrorHandler:^(__unused NSError *error) {}];
    dispatch_semaphore_t completed = dispatch_semaphore_create(0);
    __block MTLSharedEventHandle *handle = nil;
    [proxy sharedEventHandleForChannel:channel reply:^(MTLSharedEventHandle *value, uint64_t value_registry_id) {
        handle = value;
        if (registry_id)
            *registry_id = value_registry_id;
        dispatch_semaphore_signal(completed);
    }];
    if (dispatch_semaphore_wait(completed, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC)) != 0)
        return NULL;
    return handle ? (__bridge_retained void *)handle : NULL;
}

bool photon_presentation_xpc_wait_for_consumer_submission(void *opaque_connection, const char *channel_name, uint64_t frame_id)
{
    if (!opaque_connection)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    NSXPCConnection *connection = ((PhotonPresentationConnection *)opaque_connection)->object_connection;
    id<PhotonPresentationEventChannel> proxy = (id<PhotonPresentationEventChannel>)[connection remoteObjectProxyWithErrorHandler:^(__unused NSError *error) {}];
    dispatch_semaphore_t completed = dispatch_semaphore_create(0);
    [proxy producerWaitForConsumerSubmissionOnChannel:channel frameID:frame_id reply:^{
        dispatch_semaphore_signal(completed);
    }];
    return dispatch_semaphore_wait(completed, dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC)) == 0;
}

bool photon_presentation_xpc_notify_consumer_submission(void *opaque_connection, const char *channel_name, uint64_t frame_id)
{
    if (!opaque_connection)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    NSXPCConnection *connection = ((PhotonPresentationConnection *)opaque_connection)->object_connection;
    id<PhotonPresentationEventChannel> proxy = (id<PhotonPresentationEventChannel>)[connection remoteObjectProxyWithErrorHandler:^(__unused NSError *error) {}];
    dispatch_semaphore_t completed = dispatch_semaphore_create(0);
    __block BOOL accepted = NO;
    [proxy consumerSubmittedWaitOnChannel:channel frameID:frame_id reply:^(BOOL value) {
        accepted = value;
        dispatch_semaphore_signal(completed);
    }];
    return dispatch_semaphore_wait(completed, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC)) == 0 && accepted;
}

bool photon_presentation_xpc_register_backing(void *opaque_connection, const char *channel_name, uint64_t backing_id, uint64_t generation, uint32_t width, uint32_t height, uint32_t pixel_format, mach_port_t mach_port)
{
    if (!opaque_connection || mach_port == MACH_PORT_NULL || backing_id == 0 || generation == 0 || width == 0 || height == 0)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;

    PhotonPresentationConnection *pair = opaque_connection;
    xpc_object_t request = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(request, "op", "register");
    xpc_dictionary_set_string(request, "channel", channel.UTF8String);
    xpc_dictionary_set_uint64(request, "backing", backing_id);
    xpc_dictionary_set_uint64(request, "generation", generation);
    xpc_dictionary_set_uint64(request, "width", width);
    xpc_dictionary_set_uint64(request, "height", height);
    xpc_dictionary_set_uint64(request, "pixel_format", pixel_format);
    xpc_dictionary_set_mach_send(request, "port", mach_port);
    xpc_object_t response = xpc_connection_send_message_with_reply_sync(pair->surface_connection, request);
    bool accepted = response && xpc_get_type(response) == XPC_TYPE_DICTIONARY && xpc_dictionary_get_bool(response, "accepted");
    return accepted;
}

bool photon_presentation_xpc_copy_backing(void *opaque_connection, const char *channel_name, uint64_t backing_id, uint64_t generation, mach_port_t *mach_port, uint32_t *width, uint32_t *height, uint32_t *pixel_format)
{
    if (!opaque_connection || !mach_port || !width || !height || !pixel_format)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    PhotonPresentationConnection *pair = opaque_connection;
    xpc_object_t request = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(request, "op", "copy");
    xpc_dictionary_set_string(request, "channel", channel.UTF8String);
    xpc_dictionary_set_uint64(request, "backing", backing_id);
    xpc_dictionary_set_uint64(request, "generation", generation);
    xpc_object_t response = xpc_connection_send_message_with_reply_sync(pair->surface_connection, request);
    if (!response || xpc_get_type(response) != XPC_TYPE_DICTIONARY || !xpc_dictionary_get_bool(response, "accepted")) {
        return false;
    }
    mach_port_t received_port = xpc_dictionary_copy_mach_send(response, "port");
    if (received_port == MACH_PORT_NULL) {
        return false;
    }
    *mach_port = received_port;
    *width = (uint32_t)xpc_dictionary_get_uint64(response, "width");
    *height = (uint32_t)xpc_dictionary_get_uint64(response, "height");
    *pixel_format = (uint32_t)xpc_dictionary_get_uint64(response, "pixel_format");
    return true;
}

bool photon_presentation_xpc_unregister_backing(void *opaque_connection, const char *channel_name, uint64_t backing_id, uint64_t generation)
{
    if (!opaque_connection || !channel_name || backing_id == 0 || generation == 0)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    PhotonPresentationConnection *pair = opaque_connection;
    xpc_object_t request = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(request, "op", "unregister");
    xpc_dictionary_set_string(request, "channel", channel.UTF8String);
    xpc_dictionary_set_uint64(request, "backing", backing_id);
    xpc_dictionary_set_uint64(request, "generation", generation);
    xpc_object_t response = xpc_connection_send_message_with_reply_sync(pair->surface_connection, request);
    return response && xpc_get_type(response) == XPC_TYPE_DICTIONARY && xpc_dictionary_get_bool(response, "accepted");
}

static bool photon_presentation_xpc_frame_operation(void *opaque_connection, const char *channel_name, const char *operation, uint64_t backing_id, uint64_t generation, uint64_t frame_id)
{
    if (!opaque_connection || !channel_name || !operation || backing_id == 0 || generation == 0 || frame_id == 0)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    PhotonPresentationConnection *pair = opaque_connection;
    xpc_object_t request = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(request, "op", operation);
    xpc_dictionary_set_string(request, "channel", channel.UTF8String);
    xpc_dictionary_set_uint64(request, "backing", backing_id);
    xpc_dictionary_set_uint64(request, "generation", generation);
    xpc_dictionary_set_uint64(request, "frame", frame_id);
    xpc_object_t response = xpc_connection_send_message_with_reply_sync(pair->surface_connection, request);
    return response && xpc_get_type(response) == XPC_TYPE_DICTIONARY && xpc_dictionary_get_bool(response, "accepted");
}

bool photon_presentation_xpc_release_frame(void *opaque_connection, const char *channel_name, uint64_t backing_id, uint64_t generation, uint64_t frame_id)
{
    return photon_presentation_xpc_frame_operation(opaque_connection, channel_name, "release", backing_id, generation, frame_id);
}

bool photon_presentation_xpc_wait_for_frame_release(void *opaque_connection, const char *channel_name, uint64_t backing_id, uint64_t generation, uint64_t frame_id)
{
    return photon_presentation_xpc_frame_operation(opaque_connection, channel_name, "wait_release", backing_id, generation, frame_id);
}

bool photon_presentation_xpc_publish_frame(void *opaque_connection, const char *channel_name, uint64_t backing_id, uint64_t generation, uint64_t frame_id, uint64_t signal_value)
{
    if (!opaque_connection || !channel_name || backing_id == 0 || generation == 0 || frame_id == 0 || signal_value == 0)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    PhotonPresentationConnection *pair = opaque_connection;
    xpc_object_t request = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(request, "op", "publish_frame");
    xpc_dictionary_set_string(request, "channel", channel.UTF8String);
    xpc_dictionary_set_uint64(request, "backing", backing_id);
    xpc_dictionary_set_uint64(request, "generation", generation);
    xpc_dictionary_set_uint64(request, "frame", frame_id);
    xpc_dictionary_set_uint64(request, "signal_value", signal_value);
    xpc_object_t response = xpc_connection_send_message_with_reply_sync(pair->surface_connection, request);
    return response && xpc_get_type(response) == XPC_TYPE_DICTIONARY && xpc_dictionary_get_bool(response, "accepted");
}

bool photon_presentation_xpc_wait_for_frame(void *opaque_connection, const char *channel_name, uint64_t *backing_id, uint64_t *generation, uint64_t *frame_id, uint64_t *signal_value)
{
    if (!opaque_connection || !channel_name || !backing_id || !generation || !frame_id || !signal_value)
        return false;
    NSString *channel = PhotonString(channel_name);
    if (!channel)
        return false;
    PhotonPresentationConnection *pair = opaque_connection;
    xpc_object_t request = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(request, "op", "wait_frame");
    xpc_dictionary_set_string(request, "channel", channel.UTF8String);
    xpc_object_t response = xpc_connection_send_message_with_reply_sync(pair->surface_connection, request);
    if (!response || xpc_get_type(response) != XPC_TYPE_DICTIONARY || !xpc_dictionary_get_bool(response, "accepted"))
        return false;
    *backing_id = xpc_dictionary_get_uint64(response, "backing");
    *generation = xpc_dictionary_get_uint64(response, "generation");
    *frame_id = xpc_dictionary_get_uint64(response, "frame");
    *signal_value = xpc_dictionary_get_uint64(response, "signal_value");
    return *backing_id != 0 && *generation != 0 && *frame_id != 0 && *signal_value != 0;
}

void photon_presentation_xpc_release_object(void *opaque_object)
{
    if (opaque_object)
        CFBridgingRelease(opaque_object);
}

int photon_presentation_xpc_run_service(const char *service_name)
{
    @autoreleasepool {
        NSString *name = PhotonString(service_name);
        if (!name)
            return 2;
        NSString *surfaceName = [name stringByAppendingString:@".iosurface"];
        PhotonStartSurfaceService(surfaceName);
        PhotonPresentationEventListenerDelegate *delegate = [[PhotonPresentationEventListenerDelegate alloc] init];
        NSXPCListener *listener = [[NSXPCListener alloc] initWithMachServiceName:name];
        listener.delegate = delegate;
        [listener resume];
        [[NSRunLoop mainRunLoop] run];
        [listener invalidate];
        return 0;
    }
}
