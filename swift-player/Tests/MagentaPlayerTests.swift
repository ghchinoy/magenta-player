import XCTest

@testable import MagentaPlayer

final class PlayerManagerTests: XCTestCase {
    var sut: PlayerManager!
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        sut = PlayerManager()
    }
    
    override func tearDownWithError() throws {
        sut = nil
        try super.tearDownWithError()
    }
    
    func testInitialState() {
        XCTAssertFalse(sut.state.isPlaying)
        XCTAssertEqual(sut.state.modelName, "Not Loaded")
        XCTAssertNil(sut.state.engine)
    }
    
    func testTogglePlay() {
        XCTAssertFalse(sut.state.isPlaying)
        
        sut.togglePlay()
        
        XCTAssertTrue(sut.state.isPlaying)
        
        sut.togglePlay()
        
        XCTAssertFalse(sut.state.isPlaying)
    }
}
